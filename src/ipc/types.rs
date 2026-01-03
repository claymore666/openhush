//! IPC message types for daemon communication.

use serde::{Deserialize, Serialize};

/// Commands sent from TUI/GUI to daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    /// Get daemon status.
    Status,

    /// Stop the daemon.
    Stop,

    /// Load the Whisper model into GPU memory.
    LoadModel,

    /// Unload the Whisper model to free GPU memory.
    UnloadModel,

    /// Start recording audio.
    StartRecording,

    /// Stop recording audio.
    StopRecording,

    /// Toggle recording state.
    ToggleRecording,

    /// Subscribe to events.
    Subscribe {
        /// Event types to subscribe to (empty = all).
        #[serde(default)]
        events: Vec<String>,
    },

    /// Unsubscribe from events.
    Unsubscribe,

    /// Get transcription history.
    HistoryList {
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },

    /// Get configuration value.
    ConfigGet { key: String },

    /// Set configuration value.
    ConfigSet { key: String, value: String },

    /// Ping (for connection health check).
    Ping,
}

fn default_limit() -> usize {
    20
}

/// Response from daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<IpcResponseData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response data variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcResponseData {
    Status(DaemonStatus),
    Subscribed {
        subscription_id: u64,
    },
    Config {
        value: String,
    },
    History {
        items: Vec<HistoryItem>,
        total: usize,
    },
    Pong {
        timestamp: u64,
    },
    Empty {},
}

/// Daemon status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Current state (idle, recording, processing).
    pub state: DaemonState,
    /// Duration of current recording in seconds (if recording).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_duration: Option<f64>,
    /// Number of items in transcription queue.
    pub queue_depth: usize,
    /// Current Whisper model name.
    pub model: String,
    /// Whether model is loaded in GPU memory.
    pub model_loaded: bool,
    /// Current input device name.
    pub input_device: String,
    /// Enabled output modes.
    pub outputs_enabled: Vec<String>,
    /// Daemon version.
    pub version: String,
}

/// Daemon state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonState {
    Idle,
    Recording,
    Processing,
}

/// Transcription history item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: i64,
    pub timestamp: String,
    pub text: String,
    pub duration_secs: f64,
    pub llm_corrected: bool,
}

#[allow(dead_code)]
impl IpcResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            data: Some(IpcResponseData::Empty {}),
            error: None,
        }
    }

    pub fn status(status: DaemonStatus) -> Self {
        Self {
            ok: true,
            data: Some(IpcResponseData::Status(status)),
            error: None,
        }
    }

    /// Simple status response for backward compatibility.
    pub fn status_simple(is_recording: bool, model_loaded: bool) -> Self {
        Self::status(DaemonStatus {
            state: if is_recording {
                DaemonState::Recording
            } else {
                DaemonState::Idle
            },
            recording_duration: None,
            queue_depth: 0,
            model: String::new(),
            model_loaded,
            input_device: String::new(),
            outputs_enabled: Vec::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    pub fn subscribed(subscription_id: u64) -> Self {
        Self {
            ok: true,
            data: Some(IpcResponseData::Subscribed { subscription_id }),
            error: None,
        }
    }

    pub fn pong() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            ok: true,
            data: Some(IpcResponseData::Pong { timestamp }),
            error: None,
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.to_string()),
        }
    }
}

/// Events pushed from daemon to subscribed clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum IpcEvent {
    /// Recording started.
    RecordingStarted {
        recording_id: u64,
        timestamp: String,
    },

    /// Recording stopped.
    RecordingStopped {
        recording_id: u64,
        duration_secs: f64,
    },

    /// Audio level update (for visualization).
    AudioLevel {
        /// RMS level in dB (typically -60 to 0).
        rms_db: f32,
        /// Peak level in dB.
        peak_db: f32,
        /// Whether voice activity detected.
        vad_active: bool,
    },

    /// Transcription processing started.
    TranscriptionStarted { recording_id: u64 },

    /// Transcription completed.
    TranscriptionComplete {
        id: i64,
        recording_id: u64,
        text: String,
        duration_secs: f64,
        llm_corrected: bool,
    },

    /// State changed.
    StateChanged { state: DaemonState },

    /// Error occurred.
    Error { code: String, message: String },

    /// Model loading progress.
    ModelProgress {
        model: String,
        progress: f32,
        status: String,
    },

    /// Daemon shutting down.
    Shutdown,
}

/// Message wrapper for IPC protocol.
/// Allows mixing commands/responses with events on the same connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    /// Command from client to daemon.
    Command { id: u64, cmd: IpcCommand },
    /// Response from daemon to client.
    Response { id: u64, response: IpcResponse },
    /// Event pushed from daemon (no id, not a response).
    Event { event: IpcEvent },
}
