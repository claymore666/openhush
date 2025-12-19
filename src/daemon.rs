//! Background daemon for voice-to-text transcription.
//!
//! The daemon:
//! 1. Loads and keeps the Whisper model in memory
//! 2. Listens for hotkey events
//! 3. Captures audio while hotkey is held (via always-on ring buffer)
//! 4. Queues recordings for async transcription
//! 5. Outputs text to clipboard and/or pastes at cursor (in order)

use crate::config::Config;
use crate::engine::{WhisperEngine, WhisperError};
#[cfg(target_os = "linux")]
use crate::gui;
use crate::input::{AudioMark, AudioRecorder, AudioRecorderError, HotkeyEvent, HotkeyListener};
use crate::output::{OutputError, OutputHandler};
use crate::platform::{CurrentPlatform, Platform};
use crate::queue::{worker::spawn_worker, TranscriptionJob, TranscriptionTracker};
#[cfg(target_os = "linux")]
use crate::tray::{TrayEvent, TrayManager};
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[cfg(target_os = "linux")]
#[allow(clippy::single_component_path_imports)]
use gtk;

#[cfg(unix)]
use daemonize::Daemonize;

/// Channel buffer size for job and result queues
const CHANNEL_BUFFER_SIZE: usize = 32;

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

    #[cfg(unix)]
    #[error("Daemonization failed: {0}")]
    DaemonizeFailed(String),
}

/// Daemon state machine
///
/// Note: There is no "Transcribing" state anymore - transcription happens
/// asynchronously in a background worker thread.
#[derive(Debug, Clone)]
enum DaemonState {
    /// Waiting for hotkey press
    Idle,
    /// Recording audio (hotkey held) with streaming chunk support
    Recording {
        /// Original mark for sequence_id
        mark: AudioMark,
        /// Position of last emitted chunk (or mark.position initially)
        last_chunk_pos: usize,
        /// Next chunk ID (0, 1, 2, ...)
        next_chunk_id: u32,
    },
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
    pub async fn run_loop(&mut self, enable_tray: bool) -> Result<(), DaemonError> {
        info!(
            "OpenHush daemon started (display: {})",
            self.platform.display_server()
        );
        info!("Hotkey: {}", self.config.hotkey.key);
        info!("Model: {}", self.config.transcription.model);

        // Initialize system tray if enabled (Linux only for now)
        #[cfg(target_os = "linux")]
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

        #[cfg(not(target_os = "linux"))]
        let _tray_enabled = enable_tray; // Suppress unused warning
        #[cfg(not(target_os = "linux"))]
        if enable_tray {
            info!("System tray not yet supported on this platform");
        }

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

        let use_gpu = self.config.transcription.device.to_lowercase() != "cpu";
        info!("Loading Whisper model (GPU: {})...", use_gpu);
        let engine = WhisperEngine::new(
            &model_path,
            &self.config.transcription.language,
            self.config.transcription.translate,
            use_gpu,
        )?;
        info!(
            "Model loaded successfully (translate={}, device={})",
            self.config.transcription.translate, self.config.transcription.device
        );

        // Determine chunk interval (auto-tune or configured)
        let configured_interval = self.config.queue.chunk_interval_secs;
        let chunk_interval_secs = if configured_interval <= 0.0 {
            // Auto-tune mode: run GPU benchmark to determine optimal interval
            match engine.benchmark(self.config.queue.chunk_safety_margin) {
                Ok(result) => {
                    info!(
                        "Auto-tuned chunk interval: {:.2}s (GPU overhead: {:.2}s)",
                        result.recommended_chunk_interval, result.overhead_secs
                    );
                    result.recommended_chunk_interval
                }
                Err(e) => {
                    warn!(
                        "GPU benchmark failed ({}), using fallback interval of 5.0s",
                        e
                    );
                    5.0 // Fallback to safe default
                }
            }
        } else {
            info!(
                "Using configured chunk interval: {:.2}s",
                configured_interval
            );
            configured_interval
        };

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

        // Initialize always-on audio recorder with ring buffer
        let prebuffer_secs = self.config.audio.prebuffer_duration_secs;
        let audio_recorder = AudioRecorder::new_always_on(prebuffer_secs)?;
        info!(
            "Always-on audio capture initialized ({:.0}s ring buffer)",
            prebuffer_secs
        );

        // Create transcription job and result channels
        let (job_tx, job_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let (result_tx, mut result_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);

        // Spawn transcription worker in dedicated thread
        let audio_config = self.config.audio.clone();
        let _worker_handle = spawn_worker(engine, job_rx, result_tx, audio_config)?;
        info!("Transcription worker started");

        // Result tracker for ordered output
        let mut tracker = TranscriptionTracker::new();

        // Initialize hotkey listener
        let (hotkey_listener, mut hotkey_rx) = HotkeyListener::new(&self.config.hotkey.key)?;
        hotkey_listener.start()?;

        // Chunk separator (space by default) - cloned to allow config reload
        let mut chunk_separator = self.config.queue.separator.clone();

        // Streaming chunk interval (convert to Duration)
        let chunk_interval = if chunk_interval_secs > 0.0 {
            Some(tokio::time::Duration::from_secs_f32(chunk_interval_secs))
        } else {
            None
        };
        info!(
            "Streaming mode: {}",
            if chunk_interval.is_some() {
                format!("chunks every {:.1}s", chunk_interval_secs)
            } else {
                "disabled".to_string()
            }
        );

        // Chunk timer (resets on each recording start)
        let mut chunk_timer: Option<tokio::time::Interval> = None;

        info!(
            "Daemon running. Hold {} to record, release to transcribe.",
            self.config.hotkey.key
        );

        // Set up Unix signal handlers (SIGTERM, SIGHUP)
        #[cfg(unix)]
        let (mut sigterm, mut sighup) = setup_signal_handlers();

        // Main event loop
        loop {
            // Poll Unix signals (non-blocking, at start of loop)
            #[cfg(unix)]
            {
                // Use try_recv pattern via select with zero timeout
                tokio::select! {
                    biased;
                    _ = sigterm.recv() => {
                        info!("Shutdown signal received (SIGTERM)");
                        break;
                    }
                    _ = sighup.recv() => {
                        info!("SIGHUP received, reloading configuration...");
                        match Config::load() {
                            Ok(new_config) => {
                                chunk_separator = new_config.queue.separator.clone();
                                self.config = new_config;
                                info!("Configuration reloaded successfully");
                            }
                            Err(e) => {
                                error!("Failed to reload configuration: {}", e);
                            }
                        }
                        continue; // Don't break, continue with new config
                    }
                    // Immediate timeout to make this non-blocking
                    _ = tokio::time::sleep(std::time::Duration::ZERO) => {}
                }
            }

            // Process GTK events and tray (Linux only)
            #[cfg(target_os = "linux")]
            {
                // Process GTK events (required for tray icon on Linux)
                if tray.is_some() {
                    while gtk::events_pending() {
                        gtk::main_iteration_do(false);
                    }
                }

                // Check for tray events (non-blocking)
                if let Some(ref tray) = &tray {
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
            }

            tokio::select! {
                // Handle hotkey events
                Some(event) = hotkey_rx.recv() => {
                    match event {
                        HotkeyEvent::Pressed => {
                            if matches!(self.state, DaemonState::Idle) {
                                // Mark the current position in the ring buffer (instant!)
                                let mark = audio_recorder.mark();
                                let start_pos = audio_recorder.current_position();
                                debug!(
                                    "Hotkey pressed, marked position (sequence_id: {}, pos: {})",
                                    mark.sequence_id, start_pos
                                );

                                // Start chunk timer if streaming is enabled
                                if let Some(interval) = chunk_interval {
                                    let mut timer = tokio::time::interval(interval);
                                    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                                    // Skip the first tick (fires immediately)
                                    chunk_timer = Some(timer);
                                }

                                // Reset deduplication state for new recording
                                tracker.reset_dedup();

                                self.state = DaemonState::Recording {
                                    mark,
                                    last_chunk_pos: start_pos,
                                    next_chunk_id: 0,
                                };
                            }
                        }
                        HotkeyEvent::Released => {
                            if let DaemonState::Recording { mark, last_chunk_pos, next_chunk_id } = std::mem::replace(
                                &mut self.state,
                                DaemonState::Idle,
                            ) {
                                // Stop chunk timer
                                chunk_timer = None;

                                let current_pos = audio_recorder.current_position();
                                debug!(
                                    "Hotkey released, extracting final chunk (sequence_id: {}, chunk: {}, pos: {} -> {})",
                                    mark.sequence_id, next_chunk_id, last_chunk_pos, current_pos
                                );

                                // Extract final chunk from last position to current
                                if let Some(buffer) = audio_recorder.extract_chunk(last_chunk_pos, current_pos) {
                                    info!(
                                        "Final chunk {:.2}s (seq {}.{} FINAL)",
                                        buffer.duration_secs(),
                                        mark.sequence_id,
                                        next_chunk_id
                                    );

                                    // Track pending transcription
                                    tracker.add_pending(mark.sequence_id, next_chunk_id);

                                    // Submit final job
                                    let job = TranscriptionJob {
                                        buffer,
                                        sequence_id: mark.sequence_id,
                                        chunk_id: next_chunk_id,
                                        is_final: true,
                                    };
                                    if job_tx.send(job).await.is_err() {
                                        error!("Failed to submit final transcription job");
                                    }
                                } else if next_chunk_id == 0 {
                                    // No chunks emitted and final chunk too short
                                    warn!("Recording too short, ignoring");
                                } else {
                                    debug!("Final chunk too short, but {} chunks already emitted", next_chunk_id);
                                }

                                // Flush any buffered results now that hotkey is released
                                for ready in tracker.take_ready() {
                                    if !ready.text.is_empty() {
                                        // Add separator before chunks after the first
                                        let text = if ready.chunk_id > 0 {
                                            format!("{}{}", chunk_separator, ready.text)
                                        } else {
                                            ready.text
                                        };
                                        info!(
                                            "📝 Output (seq {}.{}, {} chars)",
                                            ready.sequence_id,
                                            ready.chunk_id,
                                            text.len()
                                        );
                                        if let Err(e) = output_handler.output(&text) {
                                            error!("Output failed: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle transcription results from worker
                Some(result) = result_rx.recv() => {
                    debug!(
                        "Received transcription result (seq {}.{}, {} chars)",
                        result.sequence_id,
                        result.chunk_id,
                        result.text.len()
                    );

                    // Add to tracker
                    tracker.add_result(result);

                    // Only output when NOT recording (hotkey released)
                    // This prevents AltGr/modifier key from affecting typed output
                    if matches!(self.state, DaemonState::Idle) {
                        for ready in tracker.take_ready() {
                            if !ready.text.is_empty() {
                                // Add separator before chunks after the first
                                let text = if ready.chunk_id > 0 {
                                    format!("{}{}", chunk_separator, ready.text)
                                } else {
                                    ready.text
                                };
                                info!(
                                    "📝 Output (seq {}.{}, {} chars)",
                                    ready.sequence_id,
                                    ready.chunk_id,
                                    text.len()
                                );
                                if let Err(e) = output_handler.output(&text) {
                                    error!("Output failed: {}", e);
                                }
                            } else {
                                debug!(
                                    "Empty transcription result (seq {}.{})",
                                    ready.sequence_id,
                                    ready.chunk_id
                                );
                            }
                        }
                    } else {
                        debug!("Buffering result while recording (will output on release)");
                    }
                }

                // Handle chunk timer tick (streaming transcription)
                _ = async {
                    if let Some(timer) = &mut chunk_timer {
                        timer.tick().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    if let DaemonState::Recording { ref mark, ref mut last_chunk_pos, ref mut next_chunk_id } = self.state {
                        let current_pos = audio_recorder.current_position();
                        debug!(
                            "Chunk timer tick (seq {}.{}, pos: {} -> {})",
                            mark.sequence_id, *next_chunk_id, *last_chunk_pos, current_pos
                        );

                        // Extract chunk from last position to current
                        if let Some(buffer) = audio_recorder.extract_chunk(*last_chunk_pos, current_pos) {
                            info!(
                                "📤 Streaming chunk {:.2}s (seq {}.{})",
                                buffer.duration_secs(),
                                mark.sequence_id,
                                *next_chunk_id
                            );

                            // Track pending transcription
                            tracker.add_pending(mark.sequence_id, *next_chunk_id);

                            // Submit chunk job
                            let job = TranscriptionJob {
                                buffer,
                                sequence_id: mark.sequence_id,
                                chunk_id: *next_chunk_id,
                                is_final: false,
                            };
                            if job_tx.send(job).await.is_err() {
                                error!("Failed to submit chunk transcription job");
                            }

                            // Update state for next chunk
                            *last_chunk_pos = current_pos;
                            *next_chunk_id += 1;
                        } else {
                            debug!("Chunk too short, skipping");
                        }
                    }
                }

                // Handle shutdown signal (Ctrl+C)
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutdown signal received (SIGINT)");
                    break;
                }


                // Small sleep to prevent busy loop when checking tray
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {}
            }
        }

        // Cleanup
        hotkey_listener.stop();
        drop(job_tx); // Signal worker to stop
        info!("Daemon stopped");
        Ok(())
    }
}

/// Set up Unix signal handlers for the daemon
///
/// This is called from run_loop to set up SIGTERM and SIGHUP handling.
#[cfg(unix)]
pub fn setup_signal_handlers() -> (
    tokio::signal::unix::Signal,
    tokio::signal::unix::Signal,
) {
    use tokio::signal::unix::{signal, SignalKind};

    let sigterm = signal(SignalKind::terminate())
        .expect("Failed to install SIGTERM handler");
    let sighup = signal(SignalKind::hangup())
        .expect("Failed to install SIGHUP handler");

    (sigterm, sighup)
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
                        // SAFETY: kill(pid, 0) is safe - it only checks if process exists,
                        // doesn't send any signal. The pid is validated as i32 from the PID file.
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
    // Check for stale PID file and clean up if needed
    check_and_cleanup_stale_pid()?;

    if is_running() {
        return Err(DaemonError::AlreadyRunning);
    }

    let config = Config::load()?;

    if !foreground {
        #[cfg(unix)]
        {
            daemonize_process()?;
        }
        #[cfg(not(unix))]
        {
            warn!("Background mode not supported on this platform, running in foreground");
        }
    }

    write_pid()?;

    let mut daemon = Daemon::new(config)?;
    let result = daemon.run_loop(enable_tray).await;

    remove_pid()?;

    result
}

/// Perform Unix daemonization using the daemonize crate
#[cfg(unix)]
fn daemonize_process() -> Result<(), DaemonError> {
    use std::fs::File;

    // Get log directory for stdout/stderr redirection
    let log_dir = Config::data_dir().map_err(|e| DaemonError::DaemonizeFailed(e.to_string()))?;
    let _ = std::fs::create_dir_all(&log_dir);

    // Create stdout/stderr files for daemon output
    let stdout_path = log_dir.join("daemon.out");
    let stderr_path = log_dir.join("daemon.err");

    let stdout = File::create(&stdout_path)
        .map_err(|e| DaemonError::DaemonizeFailed(format!("Cannot create stdout file: {}", e)))?;
    let stderr = File::create(&stderr_path)
        .map_err(|e| DaemonError::DaemonizeFailed(format!("Cannot create stderr file: {}", e)))?;

    // Print before forking so user sees the message
    println!("Daemonizing OpenHush (logs: {:?})...", log_dir);

    // Note: We don't use daemonize's pid_file feature because we manage it ourselves
    // with atomic creation and proper cleanup
    let daemonize = Daemonize::new()
        .working_directory("/")
        .stdout(stdout)
        .stderr(stderr);

    daemonize.start().map_err(|e| {
        DaemonError::DaemonizeFailed(format!("Fork failed: {}", e))
    })?;

    // At this point, the parent has exited and we're in the child process
    // The user will see "Daemonizing..." then their shell prompt returns

    // If we get here, we're in the child process
    info!("Daemonized successfully (PID: {})", std::process::id());

    Ok(())
}

/// Check for stale PID file and clean up if the process is no longer running
fn check_and_cleanup_stale_pid() -> Result<(), DaemonError> {
    let path = match pid_file() {
        Ok(p) => p,
        Err(_) => return Ok(()), // No PID file path available, nothing to clean up
    };

    if !path.exists() {
        return Ok(());
    }

    let pid_str = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(()), // Can't read, probably doesn't exist
    };

    let pid: i32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            // Invalid PID file content, remove it
            warn!("Removing invalid PID file");
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
    };

    // Check if process is actually running
    #[cfg(unix)]
    {
        // SAFETY: kill(pid, 0) is safe - only checks if process exists
        let result = unsafe { libc::kill(pid, 0) };
        if result != 0 {
            // Process not running, this is a stale PID file
            warn!("Removing stale PID file (process {} no longer running)", pid);
            let _ = std::fs::remove_file(&path);
        }
    }

    Ok(())
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
                // SAFETY: kill(pid, SIGTERM) sends a termination signal to the process.
                // The pid is validated as i32 from our own PID file. Sending to a non-existent
                // or different process is harmless (returns error which we ignore).
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
