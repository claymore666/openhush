//! Cross-platform IPC for daemon communication.
//!
//! Provides unified IPC on all platforms:
//! - Linux: Unix domain sockets (D-Bus is also available separately)
//! - macOS: Unix domain sockets
//! - Windows: Named pipes
//!
//! Supports both request/response and push notifications (events).

use std::path::PathBuf;
use thiserror::Error;

#[cfg(windows)]
mod named_pipe;
#[cfg(unix)]
mod unix_socket;

mod server;
mod types;

#[allow(unused_imports)]
pub use server::{IpcServer, IpcServerHandle};
pub use types::*;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum IpcError {
    #[error("Failed to bind: {0}")]
    BindFailed(String),

    #[error("Failed to connect: {0}")]
    ConnectFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    RecvFailed(String),

    #[error("Daemon not running")]
    NotRunning,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Get the IPC socket/pipe path.
pub fn ipc_path() -> PathBuf {
    #[cfg(unix)]
    {
        dirs::runtime_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("openhush.sock")
    }
    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\openhush")
    }
}

/// IPC client for TUI/GUI communication with daemon.
pub struct IpcClient {
    #[cfg(unix)]
    inner: unix_socket::IpcClientInner,
    #[cfg(windows)]
    inner: named_pipe::IpcClientInner,
}

#[allow(dead_code)]
impl IpcClient {
    /// Connect to the daemon.
    pub fn connect() -> Result<Self, IpcError> {
        #[cfg(unix)]
        let inner = unix_socket::IpcClientInner::connect()?;
        #[cfg(windows)]
        let inner = named_pipe::IpcClientInner::connect()?;

        Ok(Self { inner })
    }

    /// Send a command and wait for response.
    pub fn send(&mut self, cmd: IpcCommand) -> Result<IpcResponse, IpcError> {
        self.inner.send(cmd)
    }

    /// Subscribe to events. Returns a receiver for events.
    pub fn subscribe(&mut self) -> Result<std::sync::mpsc::Receiver<IpcEvent>, IpcError> {
        self.inner.subscribe()
    }

    /// Check if there are pending events (non-blocking).
    pub fn try_recv_event(&mut self) -> Option<IpcEvent> {
        self.inner.try_recv_event()
    }
}
