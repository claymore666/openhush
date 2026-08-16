//! Unix domain socket IPC client (works on Linux and macOS).

use super::{ipc_path, IpcCommand, IpcError, IpcEvent, IpcMessage, IpcResponse};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, warn};

/// Transport layer for IPC server (for polling).
#[allow(dead_code)]
pub struct IpcServerTransport;

/// Transport layer for IPC client (for connecting).
#[allow(dead_code)]
pub struct IpcClientTransport;

/// Internal IPC client implementation.
pub struct IpcClientInner {
    stream: UnixStream,
    reader_thread: Option<JoinHandle<()>>,
    event_rx: Option<Receiver<IpcEvent>>,
    response_rx: Receiver<(u64, IpcResponse)>,
    response_tx: Sender<(u64, IpcResponse)>,
    next_id: AtomicU64,
}

#[allow(dead_code)]
impl IpcClientInner {
    /// Connect to the daemon.
    pub fn connect() -> Result<Self, IpcError> {
        let path = ipc_path();

        if !path.exists() {
            return Err(IpcError::NotRunning);
        }

        let stream = UnixStream::connect(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                IpcError::NotRunning
            } else {
                IpcError::ConnectFailed(e.to_string())
            }
        })?;

        // Short timeouts for responsive TUI - don't block the UI
        stream
            .set_read_timeout(Some(Duration::from_millis(250)))
            .ok();
        stream
            .set_write_timeout(Some(Duration::from_millis(250)))
            .ok();

        let (response_tx, response_rx) = mpsc::channel();

        Ok(Self {
            stream,
            reader_thread: None,
            event_rx: None,
            response_rx,
            response_tx,
            next_id: AtomicU64::new(1),
        })
    }

    /// Send a command and wait for response.
    pub fn send(&mut self, cmd: IpcCommand) -> Result<IpcResponse, IpcError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // If we have a reader thread, use the message protocol
        if self.reader_thread.is_some() {
            let msg = IpcMessage::Command { id, cmd };
            let json =
                serde_json::to_string(&msg).map_err(|e| IpcError::SendFailed(e.to_string()))?;

            writeln!(self.stream, "{}", json).map_err(|e| IpcError::SendFailed(e.to_string()))?;
            self.stream
                .flush()
                .map_err(|e| IpcError::SendFailed(e.to_string()))?;

            // Wait for response with matching ID (short timeout for responsive UI)
            loop {
                match self.response_rx.recv_timeout(Duration::from_millis(250)) {
                    Ok((resp_id, response)) if resp_id == id => return Ok(response),
                    Ok(_) => continue, // Wrong ID, keep waiting
                    Err(_) => {
                        return Err(IpcError::RecvFailed("Timeout waiting for response".into()))
                    }
                }
            }
        } else {
            // Legacy mode: direct request/response
            let json =
                serde_json::to_string(&cmd).map_err(|e| IpcError::SendFailed(e.to_string()))?;

            writeln!(self.stream, "{}", json).map_err(|e| IpcError::SendFailed(e.to_string()))?;
            self.stream
                .flush()
                .map_err(|e| IpcError::SendFailed(e.to_string()))?;

            let mut reader = BufReader::new(&self.stream);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|e| IpcError::RecvFailed(e.to_string()))?;

            serde_json::from_str(&line).map_err(|e| IpcError::RecvFailed(e.to_string()))
        }
    }

    /// Subscribe to events. Starts a reader thread.
    pub fn subscribe(&mut self) -> Result<Receiver<IpcEvent>, IpcError> {
        if self.event_rx.is_some() {
            return Err(IpcError::SendFailed("Already subscribed".into()));
        }

        // Send subscribe command
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = IpcMessage::Command {
            id,
            cmd: IpcCommand::Subscribe { events: vec![] },
        };
        let json = serde_json::to_string(&msg).map_err(|e| IpcError::SendFailed(e.to_string()))?;

        writeln!(self.stream, "{}", json).map_err(|e| IpcError::SendFailed(e.to_string()))?;
        self.stream
            .flush()
            .map_err(|e| IpcError::SendFailed(e.to_string()))?;

        // Clone stream for reader thread
        let stream_clone = self.stream.try_clone().map_err(IpcError::Io)?;

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel();
        let response_tx = self.response_tx.clone();

        // Start reader thread
        let handle = thread::spawn(move || {
            let mut reader = BufReader::new(stream_clone);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        debug!("IPC connection closed");
                        break;
                    }
                    Ok(_) => match serde_json::from_str::<IpcMessage>(&line) {
                        Ok(IpcMessage::Event { event }) => {
                            if event_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Ok(IpcMessage::Response { id, response }) => {
                            if response_tx.send((id, response)).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {
                            warn!("Unexpected message type");
                        }
                        Err(e) => {
                            warn!("Failed to parse IPC message: {}", e);
                        }
                    },
                    Err(e) => {
                        debug!("IPC read error: {}", e);
                        break;
                    }
                }
            }
        });

        self.reader_thread = Some(handle);
        self.event_rx = Some(event_rx);

        // Return a clone of the receiver
        // Actually we need to return the receiver we just created
        // This is a bit awkward - let's create another channel pair
        let (tx2, rx2) = mpsc::channel();

        // Swap the event_rx with a forwarding receiver
        if let Some(rx) = self.event_rx.take() {
            thread::spawn(move || {
                for event in rx {
                    if tx2.send(event).is_err() {
                        break;
                    }
                }
            });
        }

        Ok(rx2)
    }

    /// Try to receive an event (non-blocking).
    pub fn try_recv_event(&mut self) -> Option<IpcEvent> {
        self.event_rx.as_ref()?.try_recv().ok()
    }
}

impl Drop for IpcClientInner {
    fn drop(&mut self) {
        // Reader thread will exit when stream is dropped
    }
}
