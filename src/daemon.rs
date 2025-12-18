//! Background daemon for voice-to-text transcription.
//!
//! The daemon:
//! 1. Loads and keeps the Whisper model in memory
//! 2. Listens for hotkey events
//! 3. Captures audio while hotkey is held
//! 4. Queues recordings for transcription
//! 5. Outputs text to clipboard and/or pastes at cursor

use crate::config::Config;
use crate::engine::{WhisperEngine, WhisperError};
use crate::input::{AudioRecorder, AudioRecorderError, HotkeyEvent, HotkeyListener};
use crate::output::{OutputError, OutputHandler};
use crate::platform::{CurrentPlatform, Platform};
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("Platform error: {0}")]
    Platform(#[from] crate::platform::PlatformError),

    #[error("Hotkey error: {0}")]
    Hotkey(#[from] crate::input::HotkeyListenerError),

    #[error("Audio error: {0}")]
    Audio(#[from] AudioRecorderError),

    #[error("Whisper error: {0}")]
    Whisper(#[from] WhisperError),

    #[error("Output error: {0}")]
    Output(#[from] OutputError),

    #[error("Daemon already running")]
    AlreadyRunning,

    #[error("Daemon not running")]
    NotRunning,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Daemon state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonState {
    /// Waiting for hotkey press
    Idle,
    /// Recording audio (hotkey held)
    Recording,
    /// Transcribing audio
    Transcribing,
}

/// Main daemon struct
pub struct Daemon {
    config: Config,
    platform: CurrentPlatform,
    state: DaemonState,
}

impl Daemon {
    /// Create a new daemon instance
    pub fn new(config: Config) -> Result<Self, DaemonError> {
        let platform = CurrentPlatform::new()?;

        Ok(Self {
            config,
            platform,
            state: DaemonState::Idle,
        })
    }

    /// Get the path to the Whisper model file
    fn model_path(&self) -> Result<PathBuf, DaemonError> {
        let data_dir = Config::data_dir()?;
        let model_file = format!("ggml-{}.bin", self.config.transcription.model);
        let path = data_dir.join("models").join(&model_file);
        Ok(path)
    }

    /// Main daemon loop
    pub async fn run_loop(&mut self) -> Result<(), DaemonError> {
        info!(
            "OpenHush daemon started (display: {})",
            self.platform.display_server()
        );
        info!("Hotkey: {}", self.config.hotkey.key);
        info!("Model: {}", self.config.transcription.model);

        // Check if model exists
        let model_path = self.model_path()?;
        if !model_path.exists() {
            error!(
                "Model not found at: {}. Run 'openhush model download {}'",
                model_path.display(),
                self.config.transcription.model
            );
            return Err(DaemonError::Whisper(WhisperError::ModelNotFound(
                model_path,
                self.config.transcription.model.clone(),
            )));
        }

        info!("Loading Whisper model...");
        let engine = WhisperEngine::new(&model_path, &self.config.transcription.language)?;
        info!("Model loaded successfully");

        // Initialize output handler
        let output_handler = OutputHandler::new(&self.config.output);

        // Initialize audio recorder
        let mut audio_recorder = AudioRecorder::new()?;

        // Initialize hotkey listener
        let (hotkey_listener, mut hotkey_rx) =
            HotkeyListener::new(&self.config.hotkey.key)?;
        hotkey_listener.start()?;

        info!("Daemon running. Hold {} to record, release to transcribe.",
              self.config.hotkey.key);

        // Main event loop
        loop {
            tokio::select! {
                // Handle hotkey events
                Some(event) = hotkey_rx.recv() => {
                    match event {
                        HotkeyEvent::Pressed => {
                            if self.state == DaemonState::Idle {
                                debug!("Hotkey pressed, starting recording");
                                self.state = DaemonState::Recording;

                                if let Err(e) = audio_recorder.start() {
                                    error!("Failed to start recording: {}", e);
                                    self.state = DaemonState::Idle;
                                }
                            }
                        }
                        HotkeyEvent::Released => {
                            if self.state == DaemonState::Recording {
                                debug!("Hotkey released, stopping recording");
                                self.state = DaemonState::Transcribing;

                                match audio_recorder.stop() {
                                    Ok(buffer) => {
                                        info!("Recorded {:.2}s of audio", buffer.duration_secs());

                                        // Transcribe
                                        match engine.transcribe(&buffer) {
                                            Ok(result) => {
                                                info!("Transcribed {} characters", result.text.len());

                                                // Output
                                                if let Err(e) = output_handler.output(&result.text) {
                                                    error!("Output failed: {}", e);
                                                }
                                            }
                                            Err(e) => {
                                                error!("Transcription failed: {}", e);
                                            }
                                        }
                                    }
                                    Err(AudioRecorderError::TooShort) => {
                                        warn!("Recording too short, ignoring");
                                    }
                                    Err(e) => {
                                        error!("Failed to stop recording: {}", e);
                                    }
                                }

                                self.state = DaemonState::Idle;
                            }
                        }
                    }
                }

                // Handle shutdown signal
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }

        hotkey_listener.stop();
        info!("Daemon stopped");
        Ok(())
    }
}

/// Get the PID file path
fn pid_file() -> Result<PathBuf, DaemonError> {
    let runtime_dir = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .ok_or(DaemonError::Config(crate::config::ConfigError::NoConfigDir))?;

    Ok(runtime_dir.join("openhush.pid"))
}

/// Check if daemon is already running
fn is_running() -> bool {
    if let Ok(path) = pid_file() {
        if path.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&path) {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    #[cfg(unix)]
                    {
                        let result = unsafe { libc::kill(pid, 0) };
                        return result == 0;
                    }
                    #[cfg(not(unix))]
                    {
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
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

/// Remove PID file
fn remove_pid() -> Result<(), DaemonError> {
    let path = pid_file()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
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
