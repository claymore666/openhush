//! Background daemon for voice-to-text transcription.
//!
//! The daemon:
//! 1. Loads and keeps the Whisper model in memory
//! 2. Listens for hotkey events
//! 3. Captures audio while hotkey is held
//! 4. Queues recordings for transcription
//! 5. Outputs text to clipboard and/or pastes at cursor

use crate::config::{AudioConfig, Config};
use crate::engine::{WhisperEngine, WhisperError};
use crate::gui;
use crate::input::{AudioBuffer, AudioRecorder, AudioRecorderError, HotkeyEvent, HotkeyListener};
use crate::output::{OutputError, OutputHandler};
use crate::platform::{CurrentPlatform, Platform};
use crate::tray::{TrayEvent, TrayManager};
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[cfg(target_os = "linux")]
use gtk;

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

    /// Apply audio preprocessing (normalization, compression, limiter)
    fn preprocess_audio(buffer: &mut AudioBuffer, audio_config: &AudioConfig) {
        if !audio_config.preprocessing {
            return;
        }

        let rms_before = buffer.rms_db();
        debug!("Preprocessing audio (input RMS: {:.1} dB)", rms_before);

        // 1. RMS Normalization
        if audio_config.normalization.enabled {
            buffer.normalize_rms(audio_config.normalization.target_db);
        }

        // 2. Dynamic Compression
        if audio_config.compression.enabled {
            buffer.compress(
                audio_config.compression.threshold_db,
                audio_config.compression.ratio,
                audio_config.compression.attack_ms,
                audio_config.compression.release_ms,
                audio_config.compression.makeup_gain_db,
            );
        }

        // 3. Limiter (safety net)
        if audio_config.limiter.enabled {
            buffer.limit(
                audio_config.limiter.ceiling_db,
                audio_config.limiter.release_ms,
            );
        }

        let rms_after = buffer.rms_db();
        info!(
            "Audio preprocessed: {:.1} dB -> {:.1} dB",
            rms_before, rms_after
        );
    }

    /// Get the path to the Whisper model file
    fn model_path(&self) -> Result<PathBuf, DaemonError> {
        let data_dir = Config::data_dir()?;
        let model_file = format!("ggml-{}.bin", self.config.transcription.model);
        let path = data_dir.join("models").join(&model_file);
        Ok(path)
    }

    /// Main daemon loop
    pub async fn run_loop(&mut self, enable_tray: bool) -> Result<(), DaemonError> {
        info!(
            "OpenHush daemon started (display: {})",
            self.platform.display_server()
        );
        info!("Hotkey: {}", self.config.hotkey.key);
        info!("Model: {}", self.config.transcription.model);

        // Initialize system tray if enabled
        let tray: Option<TrayManager> = if enable_tray {
            match TrayManager::new() {
                Ok(t) => {
                    info!("System tray initialized");
                    Some(t)
                }
                Err(e) => {
                    warn!("System tray unavailable: {}. Continuing without tray.", e);
                    None
                }
            }
        } else {
            info!("System tray disabled");
            None
        };

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
        let engine = WhisperEngine::new(
            &model_path,
            &self.config.transcription.language,
            self.config.transcription.translate,
        )?;
        info!(
            "Model loaded successfully (translate={})",
            self.config.transcription.translate
        );

        if self.config.audio.preprocessing {
            info!(
                "Audio preprocessing enabled (normalize={}, compress={}, limit={})",
                self.config.audio.normalization.enabled,
                self.config.audio.compression.enabled,
                self.config.audio.limiter.enabled
            );
        }

        // Initialize output handler
        let output_handler = OutputHandler::new(&self.config.output);

        // Initialize audio recorder
        let mut audio_recorder = AudioRecorder::new()?;

        // Initialize hotkey listener
        let (hotkey_listener, mut hotkey_rx) = HotkeyListener::new(&self.config.hotkey.key)?;
        hotkey_listener.start()?;

        info!(
            "Daemon running. Hold {} to record, release to transcribe.",
            self.config.hotkey.key
        );

        // Main event loop
        loop {
            // Process GTK events (required for tray icon on Linux)
            #[cfg(target_os = "linux")]
            if tray.is_some() {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
            }

            // Check for tray events (non-blocking)
            if let Some(ref tray) = tray {
                if let Some(tray_event) = tray.try_recv() {
                    match tray_event {
                        TrayEvent::ShowPreferences => {
                            info!("Opening preferences from tray");
                            gui::spawn_preferences();
                        }
                        TrayEvent::Quit => {
                            info!("Quit requested from tray");
                            break;
                        }
                        TrayEvent::StatusClicked => {
                            debug!("Status clicked");
                        }
                    }
                }
            }

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
                                    Ok(mut buffer) => {
                                        info!("Recorded {:.2}s of audio", buffer.duration_secs());

                                        // Apply preprocessing if enabled
                                        Self::preprocess_audio(&mut buffer, &self.config.audio);

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

                // Small sleep to prevent busy loop when checking tray
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {}
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
pub async fn run(foreground: bool, enable_tray: bool) -> Result<(), DaemonError> {
    if is_running() {
        return Err(DaemonError::AlreadyRunning);
    }

    let config = Config::load()?;

    if !foreground {
        warn!("Background mode not yet implemented, running in foreground");
    }

    write_pid()?;

    let mut daemon = Daemon::new(config)?;
    let result = daemon.run_loop(enable_tray).await;

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
