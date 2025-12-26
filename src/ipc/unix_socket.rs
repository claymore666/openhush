//! Unix domain socket IPC (works on Linux and macOS).

use super::{ipc_path, IpcCommand, IpcError, IpcResponse};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;
use tracing::{debug, info, warn};

/// IPC server listening for commands.
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    /// Bind and create server.
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

        info!("IPC listening on {}", path.display());
        Ok(Self { listener })
    }

    /// Try to receive a command (non-blocking).
    /// Returns command and a responder function.
    pub fn try_recv(&self) -> Option<(IpcCommand, Box<dyn FnOnce(IpcResponse) + Send>)> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .ok()?;

                let mut reader = BufReader::new(stream.try_clone().ok()?);
                let mut line = String::new();

                if reader.read_line(&mut line).ok()? > 0 {
                    match serde_json::from_str::<IpcCommand>(&line) {
                        Ok(cmd) => {
                            debug!("IPC command: {:?}", cmd);
                            let responder = Box::new(move |response: IpcResponse| {
                                let mut s = stream;
                                if let Ok(json) = serde_json::to_string(&response) {
                                    let _ = writeln!(s, "{}", json);
                                    let _ = s.flush();
                                }
                            });
                            return Some((cmd, responder));
                        }
                        Err(e) => warn!("IPC parse error: {}", e),
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => debug!("IPC accept error: {}", e),
        }
        None
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let path = ipc_path();
        let _ = std::fs::remove_file(&path);
    }
}

/// IPC client for sending commands.
pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    /// Connect to daemon.
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

        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

        Ok(Self { stream })
    }

    /// Send command and get response.
    pub fn send(&mut self, cmd: IpcCommand) -> Result<IpcResponse, IpcError> {
        let json = serde_json::to_string(&cmd).map_err(|e| IpcError::SendFailed(e.to_string()))?;

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
