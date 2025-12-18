//! Background daemon for voice-to-text transcription.
//!
//! The daemon:
//! 1. Loads and keeps the Whisper model in memory
//! 2. Listens for hotkey events
//! 3. Captures audio while hotkey is held
//! 4. Queues recordings for transcription
//! 5. Outputs text to clipboard and/or pastes at cursor

use crate::config::Config;
use crate::platform::{CurrentPlatform, Platform};
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("Platform error: {0}")]
    Platform(#[from] crate::platform::PlatformError),

    #[error("Daemon already running")]
    AlreadyRunning,

    #[error("Daemon not running")]
    NotRunning,

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Transcription error: {0}")]
    Transcription(String),

    #[error("Audio error: {0}")]
    Audio(String),
}

/// Recording with sequence ID for ordered output
#[derive(Debug)]
struct Recording {
    sequence_id: u64,
    audio_data: Vec<f32>,
}

/// Transcription result with sequence ID
#[derive(Debug)]
struct TranscriptionResult {
    sequence_id: u64,
    text: String,
}

/// Daemon state
pub struct Daemon {
    config: Config,
    platform: CurrentPlatform,
    recording_tx: mpsc::Sender<Recording>,
    recording_rx: mpsc::Receiver<Recording>,
    result_tx: mpsc::Sender<TranscriptionResult>,
    result_rx: mpsc::Receiver<TranscriptionResult>,
    sequence_counter: u64,
    next_output_sequence: u64,
    pending_results: Vec<TranscriptionResult>,
}

impl Daemon {
    /// Create a new daemon instance
    pub fn new(config: Config) -> Result<Self, DaemonError> {
        let platform = CurrentPlatform::new()?;

        // Channels for recording queue and results
        let (recording_tx, recording_rx) = mpsc::channel(100);
        let (result_tx, result_rx) = mpsc::channel(100);

        Ok(Self {
            config,
            platform,
            recording_tx,
            recording_rx,
            result_tx,
            result_rx,
            sequence_counter: 0,
            next_output_sequence: 0,
            pending_results: Vec::new(),
        })
    }

    /// Get the path to the Whisper model file
    fn model_path(&self) -> Result<PathBuf, DaemonError> {
        let data_dir = Config::data_dir()?;
        let model_file = format!("ggml-{}.bin", self.config.transcription.model);
        let path = data_dir.join("models").join(&model_file);

        if !path.exists() {
            return Err(DaemonError::ModelNotFound(format!(
                "Model '{}' not found at {}. Run 'openhush model download {}'",
                self.config.transcription.model,
                path.display(),
                self.config.transcription.model
            )));
        }

        Ok(path)
    }

    /// Queue a recording for transcription
    fn queue_recording(&mut self, audio_data: Vec<f32>) -> Result<(), DaemonError> {
        let recording = Recording {
            sequence_id: self.sequence_counter,
            audio_data,
        };
        self.sequence_counter += 1;

        // Check queue limit
        if self.config.queue.max_pending > 0 {
            // TODO: Check current queue size
        }

        self.recording_tx
            .try_send(recording)
            .map_err(|_| DaemonError::Audio("Recording queue full".into()))?;

        Ok(())
    }

    /// Process transcription results in order
    fn process_results(&mut self) -> Option<String> {
        // Collect any new results
        while let Ok(result) = self.result_rx.try_recv() {
            self.pending_results.push(result);
        }

        // Sort by sequence ID
        self.pending_results.sort_by_key(|r| r.sequence_id);

        // Output results in order
        let mut output = String::new();
        while let Some(pos) = self
            .pending_results
            .iter()
            .position(|r| r.sequence_id == self.next_output_sequence)
        {
            let result = self.pending_results.remove(pos);
            if !output.is_empty() {
                output.push_str(&self.config.queue.separator);
            }
            output.push_str(&result.text);
            self.next_output_sequence += 1;
        }

        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    /// Main daemon loop
    pub async fn run_loop(&mut self) -> Result<(), DaemonError> {
        info!(
            "OpenHush daemon started (display: {})",
            self.platform.display_server()
        );
        info!("Hotkey: {}", self.config.hotkey.key);
        info!("Model: {}", self.config.transcription.model);

        // Verify model exists
        let model_path = self.model_path()?;
        info!("Model path: {}", model_path.display());

        // TODO: Load Whisper model
        // TODO: Start hotkey listener
        // TODO: Start audio capture
        // TODO: Start transcription workers

        // For now, just wait
        info!("Daemon running. Press Ctrl+C to stop.");

        // Placeholder: wait for shutdown signal
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| DaemonError::Audio(e.to_string()))?;

        info!("Shutdown signal received");
        Ok(())
    }
}

/// Get the PID file path
fn pid_file() -> Result<PathBuf, DaemonError> {
    let runtime_dir = dirs::runtime_dir()
        .or_else(|| dirs::cache_dir())
        .ok_or_else(|| DaemonError::Config(crate::config::ConfigError::NoConfigDir))?;

    Ok(runtime_dir.join("openhush.pid"))
}

/// Check if daemon is already running
fn is_running() -> bool {
    if let Ok(path) = pid_file() {
        if path.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&path) {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    // Check if process exists
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::CommandExt;
                        let result = unsafe { libc::kill(pid, 0) };
                        return result == 0;
                    }
                    #[cfg(not(unix))]
                    {
                        // On Windows, just check if PID file exists
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Write PID file
fn write_pid() -> Result<(), DaemonError> {
    let path = pid_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DaemonError::Audio(e.to_string()))?;
    }
    std::fs::write(&path, std::process::id().to_string())
        .map_err(|e| DaemonError::Audio(e.to_string()))?;
    Ok(())
}

/// Remove PID file
fn remove_pid() -> Result<(), DaemonError> {
    let path = pid_file()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| DaemonError::Audio(e.to_string()))?;
    }
    Ok(())
}

/// Start the daemon
pub async fn run(foreground: bool) -> Result<(), DaemonError> {
    if is_running() {
        return Err(DaemonError::AlreadyRunning);
    }

    let config = Config::load()?;

    if !foreground {
        // TODO: Daemonize (fork to background)
        warn!("Background mode not yet implemented, running in foreground");
    }

    write_pid()?;

    let mut daemon = Daemon::new(config)?;
    let result = daemon.run_loop().await;

    remove_pid()?;

    result
}

/// Stop the daemon
pub async fn stop() -> Result<(), DaemonError> {
    if !is_running() {
        return Err(DaemonError::NotRunning);
    }

    let path = pid_file()?;
    if let Ok(pid_str) = std::fs::read_to_string(&path) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
                info!("Sent SIGTERM to daemon (PID: {})", pid);
            }
            #[cfg(not(unix))]
            {
                error!("Stop not implemented on this platform");
            }
        }
    }

    Ok(())
}

/// Check daemon status
pub async fn status() -> Result<(), DaemonError> {
    if is_running() {
        let path = pid_file()?;
        if let Ok(pid_str) = std::fs::read_to_string(&path) {
            println!("OpenHush daemon is running (PID: {})", pid_str.trim());
        }
    } else {
        println!("OpenHush daemon is not running");
    }

    Ok(())
}
