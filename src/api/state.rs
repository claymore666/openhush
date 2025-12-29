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

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // ApiCommand Tests
    // ===================

    #[test]
    fn test_api_command_debug() {
        let cmd = ApiCommand::StartRecording;
        assert_eq!(format!("{:?}", cmd), "StartRecording");

        let cmd = ApiCommand::StopRecording;
        assert_eq!(format!("{:?}", cmd), "StopRecording");

        let cmd = ApiCommand::ToggleRecording;
        assert_eq!(format!("{:?}", cmd), "ToggleRecording");
    }

    #[test]
    fn test_api_command_clone() {
        let cmd = ApiCommand::StartRecording;
        let cloned = cmd.clone();
        assert!(matches!(cloned, ApiCommand::StartRecording));
    }

    // ===================
    // DaemonStatus Tests
    // ===================

    #[test]
    fn test_daemon_status_default() {
        let status = DaemonStatus::default();
        assert!(!status.running);
        assert!(!status.recording);
        assert_eq!(status.queue_depth, 0);
        assert!(status.model.is_empty());
    }

    #[test]
    fn test_daemon_status_clone() {
        let status = DaemonStatus {
            running: true,
            recording: true,
            queue_depth: 5,
            model: "base".to_string(),
        };
        let cloned = status.clone();
        assert_eq!(status.running, cloned.running);
        assert_eq!(status.recording, cloned.recording);
        assert_eq!(status.queue_depth, cloned.queue_depth);
        assert_eq!(status.model, cloned.model);
    }

    #[test]
    fn test_daemon_status_debug() {
        let status = DaemonStatus {
            running: true,
            recording: false,
            queue_depth: 3,
            model: "large-v3".to_string(),
        };
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("running: true"));
        assert!(debug_str.contains("recording: false"));
        assert!(debug_str.contains("queue_depth: 3"));
        assert!(debug_str.contains("large-v3"));
    }

    // ===================
    // ApiState Tests
    // ===================

    #[tokio::test]
    async fn test_api_state_new() {
        let status = Arc::new(RwLock::new(DaemonStatus::default()));
        let (tx, _rx) = mpsc::channel(10);
        let state = ApiState::new(status, tx, Some("test_hash".to_string()));

        assert_eq!(state.api_key_hash, Some("test_hash".to_string()));
    }

    #[tokio::test]
    async fn test_api_state_without_key() {
        let status = Arc::new(RwLock::new(DaemonStatus::default()));
        let (tx, _rx) = mpsc::channel(10);
        let state = ApiState::new(status, tx, None);

        assert!(state.api_key_hash.is_none());
    }

    #[tokio::test]
    async fn test_api_state_send_command() {
        let status = Arc::new(RwLock::new(DaemonStatus::default()));
        let (tx, mut rx) = mpsc::channel(10);
        let state = ApiState::new(status, tx, None);

        state.cmd_tx.send(ApiCommand::StartRecording).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, ApiCommand::StartRecording));
    }

    #[tokio::test]
    async fn test_api_state_status_update() {
        let status = Arc::new(RwLock::new(DaemonStatus::default()));
        let (tx, _rx) = mpsc::channel(10);
        let state = ApiState::new(Arc::clone(&status), tx, None);

        // Update status
        {
            let mut s = state.status.write().await;
            s.running = true;
            s.model = "medium".to_string();
        }

        // Read back
        let s = state.status.read().await;
        assert!(s.running);
        assert_eq!(s.model, "medium");
    }

    #[test]
    fn test_api_state_clone() {
        let status = Arc::new(RwLock::new(DaemonStatus::default()));
        let (tx, _rx) = mpsc::channel::<ApiCommand>(10);
        let state = ApiState::new(status, tx, Some("hash".to_string()));

        let cloned = state.clone();
        assert_eq!(state.api_key_hash, cloned.api_key_hash);
    }
}
