//! Shared state between API handlers and daemon.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Commands sent from API to daemon.
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)] // Consistent with DaemonCommand pattern
pub enum ApiCommand {
    /// Start recording
    StartRecording,
    /// Stop recording
    StopRecording,
    /// Toggle recording state
    ToggleRecording,
}

/// Current daemon status exposed to API.
#[derive(Debug, Clone, Default)]
pub struct DaemonStatus {
    /// Whether daemon is running (always true if API is responding)
    pub running: bool,
    /// Whether currently recording
    pub recording: bool,
    /// Number of pending transcription jobs
    pub queue_depth: u32,
    /// Current model name
    pub model: String,
}

/// Shared state for API handlers.
#[derive(Clone)]
pub struct ApiState {
    /// Current daemon status (read-only for handlers)
    pub status: Arc<RwLock<DaemonStatus>>,
    /// Channel to send commands to daemon
    pub cmd_tx: mpsc::Sender<ApiCommand>,
    /// API key hash for authentication (SHA-256 hex)
    pub api_key_hash: Option<String>,
}

impl ApiState {
    /// Create new API state.
    pub fn new(
        status: Arc<RwLock<DaemonStatus>>,
        cmd_tx: mpsc::Sender<ApiCommand>,
        api_key_hash: Option<String>,
    ) -> Self {
        Self {
            status,
            cmd_tx,
            api_key_hash,
        }
    }
}
