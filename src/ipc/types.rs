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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_level_event_serialization() {
        let event = IpcEvent::AudioLevel {
            rms_db: -20.5,
            peak_db: -10.2,
            vad_active: true,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"audio_level\""));
        assert!(json.contains("\"rms_db\":-20.5"));
        assert!(json.contains("\"peak_db\":-10.2"));
        assert!(json.contains("\"vad_active\":true"));

        // Roundtrip
        let parsed: IpcEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcEvent::AudioLevel {
                rms_db,
                peak_db,
                vad_active,
            } => {
                assert!((rms_db - (-20.5)).abs() < 0.001);
                assert!((peak_db - (-10.2)).abs() < 0.001);
                assert!(vad_active);
            }
            _ => panic!("Expected AudioLevel event"),
        }
    }

    #[test]
    fn test_audio_level_event_in_message() {
        let msg = IpcMessage::Event {
            event: IpcEvent::AudioLevel {
                rms_db: -30.0,
                peak_db: -15.0,
                vad_active: false,
            },
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"event\""));
        assert!(json.contains("\"event\":\"audio_level\""));

        // Roundtrip
        let parsed: IpcMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcMessage::Event { event } => match event {
                IpcEvent::AudioLevel {
                    rms_db,
                    peak_db,
                    vad_active,
                } => {
                    assert!((rms_db - (-30.0)).abs() < 0.001);
                    assert!((peak_db - (-15.0)).abs() < 0.001);
                    assert!(!vad_active);
                }
                _ => panic!("Expected AudioLevel event"),
            },
            _ => panic!("Expected Event message"),
        }
    }

    #[test]
    fn test_status_response_serialization() {
        let response = IpcResponse::status_simple(true, true);

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"state\":\"recording\""));
        assert!(json.contains("\"model_loaded\":true"));

        // Roundtrip
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        match parsed.data {
            Some(IpcResponseData::Status(status)) => {
                assert_eq!(status.state, DaemonState::Recording);
                assert!(status.model_loaded);
            }
            _ => panic!("Expected Status data"),
        }
    }

    #[test]
    fn test_subscribe_command_serialization() {
        let cmd = IpcCommand::Subscribe { events: vec![] };

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"cmd\":\"subscribe\""));

        // Roundtrip
        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcCommand::Subscribe { events } => {
                assert!(events.is_empty());
            }
            _ => panic!("Expected Subscribe command"),
        }
    }

    #[test]
    fn test_command_message_serialization() {
        let msg = IpcMessage::Command {
            id: 42,
            cmd: IpcCommand::Status,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"command\""));
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"cmd\":\"status\""));

        // Roundtrip
        let parsed: IpcMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcMessage::Command { id, cmd } => {
                assert_eq!(id, 42);
                assert!(matches!(cmd, IpcCommand::Status));
            }
            _ => panic!("Expected Command message"),
        }
    }

    #[test]
    fn test_pong_response() {
        let response = IpcResponse::pong();

        assert!(response.ok);
        match response.data {
            Some(IpcResponseData::Pong { timestamp }) => {
                // Timestamp should be reasonable (after year 2020)
                assert!(timestamp > 1577836800000); // Jan 1, 2020 in ms
            }
            _ => panic!("Expected Pong data"),
        }
    }

    #[test]
    fn test_error_response() {
        let response = IpcResponse::error("Something went wrong");

        assert!(!response.ok);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_transcription_complete_event() {
        let event = IpcEvent::TranscriptionComplete {
            id: 123,
            recording_id: 456,
            text: "Hello world".to_string(),
            duration_secs: 2.5,
            llm_corrected: true,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"transcription_complete\""));
        assert!(json.contains("\"text\":\"Hello world\""));
        assert!(json.contains("\"llm_corrected\":true"));

        // Roundtrip
        let parsed: IpcEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcEvent::TranscriptionComplete {
                id,
                recording_id,
                text,
                duration_secs,
                llm_corrected,
            } => {
                assert_eq!(id, 123);
                assert_eq!(recording_id, 456);
                assert_eq!(text, "Hello world");
                assert!((duration_secs - 2.5).abs() < 0.001);
                assert!(llm_corrected);
            }
            _ => panic!("Expected TranscriptionComplete event"),
        }
    }

    #[test]
    fn test_daemon_state_serialization() {
        assert_eq!(
            serde_json::to_string(&DaemonState::Idle).unwrap(),
            "\"idle\""
        );
        assert_eq!(
            serde_json::to_string(&DaemonState::Recording).unwrap(),
            "\"recording\""
        );
        assert_eq!(
            serde_json::to_string(&DaemonState::Processing).unwrap(),
            "\"processing\""
        );
    }
}
