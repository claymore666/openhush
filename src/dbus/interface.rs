//! D-Bus interface definition for OpenHush daemon.
//!
//! Provides the `org.openhush.Daemon1` interface for remote control.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use zbus::interface;

/// Commands that can be sent to the daemon via D-Bus.
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum DaemonCommand {
    StartRecording,
    StopRecording,
    ToggleRecording,
    /// Load the Whisper model into GPU memory
    LoadModel,
    /// Unload the Whisper model to free GPU memory
    UnloadModel,
}

/// Shared state exposed via D-Bus properties.
#[derive(Debug, Clone, Default)]
pub struct DaemonStatus {
    pub is_recording: bool,
    pub queue_depth: u32,
    /// Whether the Whisper model is currently loaded
    pub model_loaded: bool,
}

/// D-Bus interface implementation for the daemon.
///
/// This struct is registered on the session bus at `/org/openhush/Daemon1`
/// with the interface name `org.openhush.Daemon1`.
pub struct DaemonInterface {
    /// Channel to send commands to the daemon's main loop.
    command_tx: mpsc::Sender<DaemonCommand>,
    /// Shared state for reading properties.
    status: Arc<RwLock<DaemonStatus>>,
}

impl DaemonInterface {
    /// Create a new D-Bus interface.
    pub fn new(command_tx: mpsc::Sender<DaemonCommand>, status: Arc<RwLock<DaemonStatus>>) -> Self {
        Self { command_tx, status }
    }
}

#[interface(name = "org.openhush.Daemon1")]
impl DaemonInterface {
    /// Start recording audio.
    async fn start_recording(&self) -> zbus::fdo::Result<()> {
        self.command_tx
            .send(DaemonCommand::StartRecording)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to send command: {}", e)))?;
        Ok(())
    }

    /// Stop recording audio.
    async fn stop_recording(&self) -> zbus::fdo::Result<()> {
        self.command_tx
            .send(DaemonCommand::StopRecording)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to send command: {}", e)))?;
        Ok(())
    }

    /// Toggle recording state.
    async fn toggle_recording(&self) -> zbus::fdo::Result<()> {
        self.command_tx
            .send(DaemonCommand::ToggleRecording)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to send command: {}", e)))?;
        Ok(())
    }

    /// Load the Whisper model into GPU memory.
    async fn load_model(&self) -> zbus::fdo::Result<()> {
        self.command_tx
            .send(DaemonCommand::LoadModel)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to send command: {}", e)))?;
        Ok(())
    }

    /// Unload the Whisper model to free GPU memory.
    async fn unload_model(&self) -> zbus::fdo::Result<()> {
        self.command_tx
            .send(DaemonCommand::UnloadModel)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to send command: {}", e)))?;
        Ok(())
    }

    /// Get current daemon status as a string.
    async fn get_status(&self) -> String {
        let status = self.status.read().await;
        if status.is_recording {
            "recording".to_string()
        } else if status.model_loaded {
            "idle".to_string()
        } else {
            "standby".to_string()
        }
    }

    /// Whether the daemon is currently recording.
    #[zbus(property)]
    async fn is_recording(&self) -> bool {
        self.status.read().await.is_recording
    }

    /// Number of transcriptions in the queue.
    #[zbus(property)]
    async fn queue_depth(&self) -> u32 {
        self.status.read().await.queue_depth
    }

    /// Whether the Whisper model is currently loaded.
    #[zbus(property)]
    async fn model_loaded(&self) -> bool {
        self.status.read().await.model_loaded
    }

    /// Daemon version.
    #[zbus(property)]
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    // Signals are emitted via SignalContext, not defined here.
    // See service.rs for signal emission.
}
