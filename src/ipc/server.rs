//! IPC server for daemon with event broadcasting.

use super::{ipc_path, IpcCommand, IpcError, IpcEvent, IpcMessage, IpcResponse};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tracing::{debug, error, info, warn};

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// Handle to the IPC server for broadcasting events.
#[derive(Clone)]
#[allow(dead_code)]
pub struct IpcServerHandle {
    /// Sender for broadcasting events to all subscribed clients.
    event_tx: std::sync::mpsc::Sender<IpcEvent>,
    /// Flag to signal shutdown.
    shutdown: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl IpcServerHandle {
    /// Broadcast an event to all subscribed clients.
    pub fn broadcast(&self, event: IpcEvent) {
        if self.event_tx.send(event).is_err() {
            debug!("No IPC clients connected for broadcast");
        }
    }

    /// Signal server shutdown.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// IPC server that accepts connections and handles commands.
#[allow(dead_code)]
pub struct IpcServer {
    handle: IpcServerHandle,
    event_rx: std::sync::mpsc::Receiver<IpcEvent>,
    #[cfg(unix)]
    listener: UnixListener,
    /// Connected clients with their subscription status.
    clients: Arc<Mutex<HashMap<u64, ClientConnection>>>,
    /// Next client ID.
    next_client_id: AtomicU64,
    /// Command handler callback.
    command_handler: Option<Box<dyn Fn(IpcCommand) -> IpcResponse + Send + Sync>>,
    /// Server thread handle.
    thread: Option<JoinHandle<()>>,
}

#[allow(dead_code)]
struct ClientConnection {
    #[cfg(unix)]
    stream: UnixStream,
    subscribed: bool,
}

#[allow(dead_code)]
impl IpcServer {
    /// Create and bind the IPC server.
    #[cfg(unix)]
    pub fn new() -> Result<Self, IpcError> {
        let path = ipc_path();

        // Remove stale socket
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let listener = UnixListener::bind(&path)
            .map_err(|e| IpcError::BindFailed(format!("{}: {}", path.display(), e)))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| IpcError::BindFailed(e.to_string()))?;

        info!("IPC server listening on {}", path.display());

        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));

        let handle = IpcServerHandle {
            event_tx,
            shutdown: shutdown.clone(),
        };

        Ok(Self {
            handle,
            event_rx,
            listener,
            clients: Arc::new(Mutex::new(HashMap::new())),
            next_client_id: AtomicU64::new(1),
            command_handler: None,
            thread: None,
        })
    }

    /// Create and bind the IPC server (stub for non-Unix).
    #[cfg(not(unix))]
    pub fn new() -> Result<Self, IpcError> {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));

        let handle = IpcServerHandle {
            event_tx,
            shutdown: shutdown.clone(),
        };

        Ok(Self {
            handle,
            event_rx,
            clients: Arc::new(Mutex::new(HashMap::new())),
            next_client_id: AtomicU64::new(1),
            command_handler: None,
            thread: None,
        })
    }

    /// Get a handle for broadcasting events.
    pub fn handle(&self) -> IpcServerHandle {
        self.handle.clone()
    }

    /// Set the command handler callback.
    pub fn set_command_handler<F>(&mut self, handler: F)
    where
        F: Fn(IpcCommand) -> IpcResponse + Send + Sync + 'static,
    {
        self.command_handler = Some(Box::new(handler));
    }

    /// Poll for incoming commands (non-blocking).
    /// Returns any received command with a responder.
    #[cfg(unix)]
    #[allow(clippy::type_complexity)]
    pub fn poll(&self) -> Vec<(u64, IpcCommand, Box<dyn FnOnce(IpcResponse) + Send>)> {
        let mut commands = Vec::new();

        // Accept new connections
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let client_id = self.next_client_id.fetch_add(1, Ordering::SeqCst);
                    stream
                        .set_nonblocking(true)
                        .expect("Failed to set non-blocking");
                    debug!("IPC client {} connected", client_id);

                    let mut clients = self.clients.lock().unwrap();
                    clients.insert(
                        client_id,
                        ClientConnection {
                            stream,
                            subscribed: false,
                        },
                    );
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    debug!("IPC accept error: {}", e);
                    break;
                }
            }
        }

        // Read from connected clients
        let mut clients = self.clients.lock().unwrap();
        let mut to_remove = Vec::new();

        for (&client_id, conn) in clients.iter_mut() {
            let mut stream_clone = match conn.stream.try_clone() {
                Ok(s) => s,
                Err(_) => {
                    to_remove.push(client_id);
                    continue;
                }
            };

            let mut reader = BufReader::new(&conn.stream);
            let mut line = String::new();

            match reader.read_line(&mut line) {
                Ok(0) => {
                    // Connection closed
                    debug!("IPC client {} disconnected", client_id);
                    to_remove.push(client_id);
                }
                Ok(_) => {
                    // Parse message
                    match serde_json::from_str::<IpcMessage>(&line) {
                        Ok(IpcMessage::Command { id, cmd }) => {
                            debug!("IPC command from client {}: {:?}", client_id, cmd);
                            let responder = Box::new(move |response: IpcResponse| {
                                let msg = IpcMessage::Response { id, response };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = writeln!(stream_clone, "{}", json);
                                    let _ = stream_clone.flush();
                                }
                            });
                            commands.push((
                                client_id,
                                cmd,
                                responder as Box<dyn FnOnce(IpcResponse) + Send>,
                            ));
                        }
                        Ok(_) => {
                            warn!("Unexpected message type from client {}", client_id);
                        }
                        Err(e) => {
                            // Try legacy format (direct command without wrapper)
                            if let Ok(cmd) = serde_json::from_str::<IpcCommand>(&line) {
                                debug!("IPC legacy command from client {}: {:?}", client_id, cmd);
                                let responder = Box::new(move |response: IpcResponse| {
                                    if let Ok(json) = serde_json::to_string(&response) {
                                        let _ = writeln!(stream_clone, "{}", json);
                                        let _ = stream_clone.flush();
                                    }
                                });
                                commands.push((
                                    client_id,
                                    cmd,
                                    responder as Box<dyn FnOnce(IpcResponse) + Send>,
                                ));
                            } else {
                                warn!("IPC parse error from client {}: {}", client_id, e);
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available
                }
                Err(e) => {
                    debug!("IPC read error from client {}: {}", client_id, e);
                    to_remove.push(client_id);
                }
            }
        }

        // Remove disconnected clients
        for client_id in to_remove {
            clients.remove(&client_id);
        }

        commands
    }

    /// Poll for incoming commands (stub for non-Unix).
    #[cfg(not(unix))]
    #[allow(clippy::type_complexity)]
    pub fn poll(&self) -> Vec<(u64, IpcCommand, Box<dyn FnOnce(IpcResponse) + Send>)> {
        // IPC not yet implemented on Windows
        Vec::new()
    }

    /// Mark a client as subscribed to events.
    pub fn subscribe_client(&self, client_id: u64) {
        if let Ok(mut clients) = self.clients.lock() {
            if let Some(conn) = clients.get_mut(&client_id) {
                conn.subscribed = true;
                debug!("IPC client {} subscribed to events", client_id);
            }
        }
    }

    /// Broadcast an event to all subscribed clients.
    #[cfg(unix)]
    pub fn broadcast_event(&self, event: &IpcEvent) {
        let msg = IpcMessage::Event {
            event: event.clone(),
        };
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize event: {}", e);
                return;
            }
        };

        let mut clients = self.clients.lock().unwrap();
        let mut to_remove = Vec::new();

        for (&client_id, conn) in clients.iter_mut() {
            if !conn.subscribed {
                continue;
            }

            if let Err(e) = writeln!(conn.stream, "{}", json) {
                debug!("Failed to send event to client {}: {}", client_id, e);
                to_remove.push(client_id);
                continue;
            }
            if let Err(e) = conn.stream.flush() {
                debug!("Failed to flush to client {}: {}", client_id, e);
                to_remove.push(client_id);
            }
        }

        // Remove failed clients
        for client_id in to_remove {
            clients.remove(&client_id);
        }
    }

    /// Broadcast an event to all subscribed clients (stub for non-Unix).
    #[cfg(not(unix))]
    pub fn broadcast_event(&self, _event: &IpcEvent) {
        // IPC not yet implemented on Windows
        debug!("IPC broadcast not implemented on this platform");
    }

    /// Process pending broadcasts from the handle.
    #[cfg(unix)]
    pub fn process_broadcasts(&self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.broadcast_event(&event);
        }
    }

    /// Process pending broadcasts from the handle (stub for non-Unix).
    #[cfg(not(unix))]
    pub fn process_broadcasts(&self) {
        // IPC not yet implemented on Windows
        while self.event_rx.try_recv().is_ok() {}
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let path = ipc_path();
        let _ = std::fs::remove_file(&path);
        info!("IPC server stopped");
    }
}
