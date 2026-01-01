//! Cross-platform IPC for daemon control on macOS and Windows.
//!
//! Linux uses D-Bus (see dbus module). This module provides alternatives:
//! - macOS: Unix domain sockets
//! - Windows: Named pipes

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[cfg(windows)]
mod named_pipe;
#[cfg(unix)]
mod unix_socket;

#[cfg(windows)]
pub use named_pipe::{IpcClient, IpcServer};
#[cfg(unix)]
pub use unix_socket::{IpcClient, IpcServer};

#[derive(Error, Debug)]
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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Commands sent to daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum IpcCommand {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "stop")]
    Stop,
    /// Load the Whisper model into GPU memory
    #[serde(rename = "load_model")]
    LoadModel,
    /// Unload the Whisper model to free GPU memory
    #[serde(rename = "unload_model")]
    UnloadModel,
    /// Start recording audio
    #[serde(rename = "start_recording")]
    StartRecording,
    /// Stop recording audio
    #[serde(rename = "stop_recording")]
    StopRecording,
    /// Toggle recording state
    #[serde(rename = "toggle_recording")]
    ToggleRecording,
}

/// Response from daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_loaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            running: None,
            recording: None,
            model_loaded: None,
            version: None,
            error: None,
        }
    }

    pub fn status(recording: bool, model_loaded: bool) -> Self {
        Self {
            ok: true,
            running: Some(true),
            recording: Some(recording),
            model_loaded: Some(model_loaded),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            error: None,
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            ok: false,
            running: None,
            recording: None,
            model_loaded: None,
            version: None,
            error: Some(msg.to_string()),
        }
    }
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
