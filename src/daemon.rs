//! Background daemon for voice-to-text transcription.
//!
//! The daemon:
//! 1. Loads and keeps the Whisper model in memory
//! 2. Listens for hotkey events
//! 3. Captures audio while hotkey is held (via always-on ring buffer)
//! 4. Queues recordings for async transcription
//! 5. Outputs text to clipboard and/or pastes at cursor (in order)

use crate::config::{Config, CorrectionConfig, VocabularyConfig};
use crate::correction::TextCorrector;
#[cfg(target_os = "linux")]
use crate::dbus::{DaemonCommand, DaemonStatus, DbusService};
use crate::engine::{WhisperEngine, WhisperError};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::gui;
use crate::input::{AudioMark, AudioRecorder, AudioRecorderError, HotkeyEvent, HotkeyListener};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::ipc::{IpcCommand, IpcResponse, IpcServer};
use crate::output::{OutputError, OutputHandler};
use crate::platform::{CurrentPlatform, Platform};
use crate::queue::{
    worker::spawn_worker, TranscriptionJob, TranscriptionResult, TranscriptionTracker,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::tray::{TrayEvent, TrayManager};
use crate::vad::VadConfig;
use crate::vad::{silero::SileroVad, VadEngine, VadError, VadState};
use crate::vocabulary::{VocabularyError, VocabularyManager};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
#[cfg(target_os = "linux")]
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Channel buffer size for job and result queues
const CHANNEL_BUFFER_SIZE: usize = 32;

/// VAD processing interval (32ms = 512 samples at 16kHz, matches Silero VAD chunk size)
const VAD_PROCESS_INTERVAL_MS: u64 = 32;

// ============================================================================
// Initialization Functions
// ============================================================================

/// Initialize Silero VAD if enabled or required for continuous mode.
#[allow(clippy::type_complexity)]
fn init_vad(
    vad_config: &VadConfig,
    is_continuous_mode: bool,
) -> Result<(Option<Box<dyn VadEngine>>, Option<VadState>), DaemonError> {
    if !vad_config.enabled && !is_continuous_mode {
        return Ok((None, None));
    }

    match SileroVad::new(vad_config) {
        Ok(vad) => {
            // Get sample rate from VAD engine instead of hardcoding
            let sample_rate = vad.sample_rate();
            info!(
                "Silero VAD initialized (threshold: {:.2}, min_silence: {}ms, min_speech: {}ms, sample_rate: {}Hz)",
                vad_config.threshold, vad_config.min_silence_ms, vad_config.min_speech_ms, sample_rate
            );
            let state = VadState::new(vad_config.clone(), sample_rate);
            Ok((Some(Box::new(vad) as Box<dyn VadEngine>), Some(state)))
        }
        Err(e) => {
            if is_continuous_mode {
                Err(DaemonError::Vad(e))
            } else {
                warn!("VAD initialization failed: {}. Continuing without VAD.", e);
                Ok((None, None))
            }
        }
    }
}

/// Initialize vocabulary manager if enabled.
async fn init_vocabulary(config: &VocabularyConfig) -> Option<Arc<VocabularyManager>> {
    if !config.enabled {
        return None;
    }

    let vocab_path = config
        .path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| VocabularyManager::default_path().unwrap_or_default());

    let manager = Arc::new(VocabularyManager::new(vocab_path.clone()));
    match manager.load().await {
        Ok(true) => {
            info!(
                "Vocabulary loaded ({} rules) from: {}",
                manager.rule_count().await,
                vocab_path.display()
            );
        }
        Ok(false) => {
            info!(
                "Vocabulary file not found: {} (will be created on first use)",
                vocab_path.display()
            );
        }
        Err(e) => {
            warn!(
                "Failed to load vocabulary: {}. Continuing without vocabulary.",
                e
            );
        }
    }
    Some(manager)
}

/// Initialize text corrector if enabled.
async fn init_corrector(config: &CorrectionConfig) -> Option<Arc<TextCorrector>> {
    if !config.enabled {
        return None;
    }

    let corrector = Arc::new(TextCorrector::new(config.clone()));

    if corrector.is_available().await {
        info!(
            "LLM correction enabled (model: {}, filler removal: {:?})",
            config.ollama_model,
            if config.remove_fillers {
                Some(config.filler_mode)
            } else {
                None
            }
        );
        Some(corrector)
    } else {
        warn!(
            "Ollama not available at {}. Continuing without LLM correction.",
            config.ollama_url
        );
        None
    }
}

// ============================================================================
// Output Processing
// ============================================================================

/// Show backpressure notification to user (Linux only).
#[cfg(unix)]
fn notify_backpressure(notify_enabled: bool) {
    if notify_enabled {
        let _ = notify_rust::Notification::new()
            .summary("OpenHush")
            .body("Transcription queue full - audio dropped")
            .show();
    }
}

/// Reload configuration from disk and update runtime state.
fn reload_config(config: &mut Config, separator: &mut String) {
    match Config::load() {
        Ok(new_config) => {
            separator.clone_from(&new_config.queue.separator);
            *config = new_config;
            info!("Configuration reloaded successfully");
        }
        Err(e) => {
            error!("Failed to reload configuration: {}", e);
        }
    }
}

/// Backpressure configuration for transcription queue management.
#[derive(Clone, Copy)]
struct BackpressureConfig {
    max_pending: u32,
    high_water_mark: u32,
    strategy: crate::config::BackpressureStrategy,
    notify: bool,
}

impl BackpressureConfig {
    fn from_queue_config(queue: &crate::config::QueueConfig) -> Self {
        Self {
            max_pending: queue.max_pending,
            high_water_mark: queue.high_water_mark,
            strategy: queue.backpressure_strategy,
            notify: queue.notify_on_backpressure,
        }
    }
}

/// Process and output a transcription result.
///
/// Applies vocabulary replacements, LLM correction, and outputs the text.
/// Returns the processed text for logging purposes.
async fn process_and_output(
    result: TranscriptionResult,
    chunk_separator: &str,
    vocabulary_manager: &Option<Arc<VocabularyManager>>,
    text_corrector: &Option<Arc<TextCorrector>>,
    output_handler: &OutputHandler,
) {
    if result.text.is_empty() {
        debug!(
            "Empty transcription result (seq {}.{})",
            result.sequence_id, result.chunk_id
        );
        return;
    }

    // Add separator before chunks after the first
    let mut text = result.text;
    if result.chunk_id > 0 {
        text.insert_str(0, chunk_separator);
    }

    // Apply vocabulary replacements
    if let Some(ref vocab) = vocabulary_manager {
        text = vocab.apply(&text).await;
    }

    // Apply LLM correction (includes filler removal)
    if let Some(ref corrector) = text_corrector {
        match corrector.correct(&text).await {
            Ok(corrected) => text = corrected,
            Err(e) => warn!("LLM correction failed: {}", e),
        }
    }

    info!(
        "📝 Output (seq {}.{}, {} chars)",
        result.sequence_id,
        result.chunk_id,
        text.len()
    );

    if let Err(e) = output_handler.output(&text) {
        error!("Output failed: {}", e);
    }
}

// ============================================================================
// Error Types
// ============================================================================

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

    #[error("VAD error: {0}")]
    Vad(#[from] VadError),

    #[error("Vocabulary error: {0}")]
    Vocabulary(#[from] VocabularyError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(unix)]
    #[error("Daemonization failed: {0}")]
    DaemonizeFailed(String),

    #[error("Transcription worker failed: channel closed")]
    WorkerFailed,
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
    /// Continuous recording with VAD-based segmentation
    ContinuousRecording {
        /// Original mark for sequence_id
        mark: AudioMark,
        /// Position of last VAD-detected speech start
        speech_start_pos: Option<usize>,
        /// Position of last processed audio
        last_vad_pos: usize,
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

        // Check accessibility permissions on macOS
        #[cfg(target_os = "macos")]
        {
            use crate::platform::PlatformError;
            if let Err(e) = platform.check_accessibility_permissions() {
                // Log the error but don't fail - macOS will prompt when needed
                warn!("Accessibility check: {}", e);
                // For permission denied, we continue but warn heavily
                if matches!(e, PlatformError::Accessibility(_)) {
                    warn!("OpenHush may not work correctly without accessibility permissions");
                }
            }
        }

        Ok(Self {
            config,
            platform,
            state: DaemonState::Idle,
        })
    }

    /// Get the path to the Whisper model file
    fn model_path(&self) -> Result<PathBuf, DaemonError> {
        let data_dir = Config::data_dir()?;
        let model_file = format!("ggml-{}.bin", self.config.transcription.effective_model());
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
        info!(
            "Model: {} (preset: {:?})",
            self.config.transcription.effective_model(),
            self.config.transcription.preset
        );

        // Initialize system tray if enabled (Linux, macOS, and Windows)
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let tray: Option<TrayManager> = if enable_tray {
            match TrayManager::new().await {
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

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let _tray_enabled = enable_tray; // Suppress unused warning
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        if enable_tray {
            info!("System tray not yet supported on this platform");
        }

        // Initialize D-Bus service (Linux only)
        #[cfg(target_os = "linux")]
        let dbus_status = Arc::new(RwLock::new(DaemonStatus::default()));
        #[cfg(target_os = "linux")]
        let (dbus_service, mut dbus_rx) = match DbusService::start(dbus_status.clone()).await {
            Ok((service, rx)) => (Some(service), Some(rx)),
            Err(e) => {
                warn!(
                    "D-Bus service unavailable: {}. Continuing without D-Bus control.",
                    e
                );
                (None, None)
            }
        };

        // Initialize IPC server (macOS and Windows)
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let ipc_server: Option<IpcServer> = match IpcServer::new() {
            Ok(server) => Some(server),
            Err(e) => {
                warn!(
                    "IPC server unavailable: {}. Continuing without IPC control.",
                    e
                );
                None
            }
        };

        // Check if model exists
        let model_path = self.model_path()?;
        let effective_model = self.config.transcription.effective_model();
        if !model_path.exists() {
            error!(
                "Model not found at: {}. Run 'openhush model download {}'",
                model_path.display(),
                effective_model
            );
            return Err(DaemonError::Whisper(WhisperError::ModelNotFound(
                model_path,
                effective_model.to_string(),
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

        if self.config.audio.noise_reduction.enabled {
            info!(
                "RNNoise noise reduction enabled (strength: {:.2})",
                self.config.audio.noise_reduction.strength
            );
        }

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
        let resampling_quality = self.config.audio.resampling_quality;
        let audio_recorder = AudioRecorder::new_always_on(prebuffer_secs, resampling_quality)?;
        info!(
            "Always-on audio capture initialized ({:.0}s ring buffer, {:?} resampling)",
            prebuffer_secs, resampling_quality
        );

        // Initialize VAD if enabled (for continuous dictation mode)
        let vad_config = self.config.vad.clone();
        let is_continuous_mode = self.config.hotkey.mode == "continuous";
        let (mut vad_engine, mut vad_state) = init_vad(&vad_config, is_continuous_mode)?;

        // Initialize vocabulary manager if enabled
        let vocabulary_manager = init_vocabulary(&self.config.vocabulary).await;

        // Vocabulary reload timer (check for file changes periodically)
        let vocab_reload_interval =
            if self.config.vocabulary.enabled && self.config.vocabulary.reload_interval_secs > 0 {
                Some(tokio::time::Duration::from_secs(
                    self.config.vocabulary.reload_interval_secs as u64,
                ))
            } else {
                None
            };
        let mut vocab_reload_timer: Option<tokio::time::Interval> =
            vocab_reload_interval.map(|d| {
                let mut timer = tokio::time::interval(d);
                timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                timer
            });

        // Initialize text corrector if enabled
        let text_corrector = init_corrector(&self.config.correction).await;

        // Create transcription job and result channels
        let (job_tx, job_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let (result_tx, mut result_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);

        // Spawn transcription worker in dedicated thread
        let audio_config = self.config.audio.clone();
        let worker_handle = spawn_worker(engine, job_rx, result_tx, audio_config)?;
        info!("Transcription worker started");

        // Result tracker for ordered output
        let mut tracker = TranscriptionTracker::new();

        // Initialize hotkey listener
        let (hotkey_listener, mut hotkey_rx) = HotkeyListener::new(&self.config.hotkey.key)?;
        hotkey_listener.start()?;

        // Chunk separator (space by default) - cloned to allow config reload
        let mut chunk_separator = self.config.queue.separator.clone();

        // Backpressure configuration
        let bp = BackpressureConfig::from_queue_config(&self.config.queue);

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

        // VAD timer (for continuous dictation mode)
        let mut vad_timer: Option<tokio::time::Interval> = None;

        if is_continuous_mode {
            info!(
                "Daemon running in CONTINUOUS mode. Press {} to start/stop VAD-based dictation.",
                self.config.hotkey.key
            );
        } else {
            info!(
                "Daemon running. Hold {} to record, release to transcribe.",
                self.config.hotkey.key
            );
        }

        // Set up Unix signal handlers (SIGTERM, SIGHUP)
        #[cfg(unix)]
        let (mut sigterm, mut sighup) = setup_signal_handlers()?;

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
                        reload_config(&mut self.config, &mut chunk_separator);
                        continue; // Don't break, continue with new config
                    }
                    // Immediate timeout to make this non-blocking
                    _ = tokio::time::sleep(std::time::Duration::ZERO) => {}
                }
            }

            // Check for tray events (Linux, macOS, and Windows)
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            {
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

            // Check for D-Bus commands (Linux only)
            #[cfg(target_os = "linux")]
            if let Some(ref mut rx) = dbus_rx {
                if let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        DaemonCommand::StartRecording => {
                            if matches!(self.state, DaemonState::Idle) {
                                info!("🎙️ Recording started via D-Bus");
                                let mark = audio_recorder.mark();
                                tracker.reset_dedup();

                                // Start chunk timer if streaming enabled
                                if let Some(interval) = chunk_interval {
                                    let mut timer = tokio::time::interval(interval);
                                    timer.set_missed_tick_behavior(
                                        tokio::time::MissedTickBehavior::Skip,
                                    );
                                    chunk_timer = Some(timer);
                                }

                                self.state = DaemonState::Recording {
                                    mark,
                                    last_chunk_pos: audio_recorder.current_position(),
                                    next_chunk_id: 0,
                                };

                                // Update D-Bus status
                                {
                                    let mut status = dbus_status.write().await;
                                    status.is_recording = true;
                                }
                                if let Some(ref service) = dbus_service {
                                    let _ = service.emit_recording_changed().await;
                                }
                            }
                        }
                        DaemonCommand::StopRecording => {
                            if !matches!(self.state, DaemonState::Idle) {
                                info!("🛑 Recording stopped via D-Bus");
                                chunk_timer = None;
                                vad_timer = None;
                                self.state = DaemonState::Idle;

                                // Update D-Bus status
                                {
                                    let mut status = dbus_status.write().await;
                                    status.is_recording = false;
                                }
                                if let Some(ref service) = dbus_service {
                                    let _ = service.emit_recording_changed().await;
                                }
                            }
                        }
                        DaemonCommand::ToggleRecording => {
                            if matches!(self.state, DaemonState::Idle) {
                                info!("🎙️ Recording toggled ON via D-Bus");
                                let mark = audio_recorder.mark();
                                tracker.reset_dedup();

                                if let Some(interval) = chunk_interval {
                                    let mut timer = tokio::time::interval(interval);
                                    timer.set_missed_tick_behavior(
                                        tokio::time::MissedTickBehavior::Skip,
                                    );
                                    chunk_timer = Some(timer);
                                }

                                self.state = DaemonState::Recording {
                                    mark,
                                    last_chunk_pos: audio_recorder.current_position(),
                                    next_chunk_id: 0,
                                };

                                {
                                    let mut status = dbus_status.write().await;
                                    status.is_recording = true;
                                }
                                if let Some(ref service) = dbus_service {
                                    let _ = service.emit_recording_changed().await;
                                }
                            } else {
                                info!("🛑 Recording toggled OFF via D-Bus");
                                chunk_timer = None;
                                vad_timer = None;
                                self.state = DaemonState::Idle;

                                {
                                    let mut status = dbus_status.write().await;
                                    status.is_recording = false;
                                }
                                if let Some(ref service) = dbus_service {
                                    let _ = service.emit_recording_changed().await;
                                }
                            }
                        }
                    }
                }
            }

            // Check for IPC commands (macOS and Windows)
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            if let Some(ref server) = ipc_server {
                if let Some((cmd, responder)) = server.try_recv() {
                    match cmd {
                        IpcCommand::Status => {
                            let is_recording = !matches!(self.state, DaemonState::Idle);
                            responder(IpcResponse::status(is_recording));
                        }
                        IpcCommand::Stop => {
                            info!("Stop command received via IPC");
                            responder(IpcResponse::ok());
                            break; // Exit the main loop
                        }
                    }
                }
            }

            tokio::select! {
                // Handle hotkey events
                Some(event) = hotkey_rx.recv() => {
                    match event {
                        HotkeyEvent::Pressed => {
                            if is_continuous_mode {
                                // Continuous mode: toggle recording on/off
                                match &self.state {
                                    DaemonState::Idle => {
                                        let mark = audio_recorder.mark();
                                        let start_pos = audio_recorder.current_position();
                                        info!("🎙️ Continuous recording started (sequence_id: {})", mark.sequence_id);

                                        // Start VAD timer for continuous processing
                                        let mut timer = tokio::time::interval(
                                            tokio::time::Duration::from_millis(VAD_PROCESS_INTERVAL_MS)
                                        );
                                        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                                        vad_timer = Some(timer);

                                        // Reset VAD state for new recording
                                        if let Some(ref mut state) = vad_state {
                                            state.reset();
                                        }
                                        if let Some(ref mut engine) = vad_engine {
                                            engine.reset();
                                        }
                                        tracker.reset_dedup();

                                        self.state = DaemonState::ContinuousRecording {
                                            mark,
                                            speech_start_pos: None,
                                            last_vad_pos: start_pos,
                                            next_chunk_id: 0,
                                        };
                                    }
                                    DaemonState::ContinuousRecording { .. } => {
                                        // Toggle off - stop continuous recording
                                        info!("🛑 Continuous recording stopped");
                                        vad_timer = None;
                                        self.state = DaemonState::Idle;
                                    }
                                    _ => {}
                                }
                            } else if matches!(self.state, DaemonState::Idle) {
                                // Push-to-talk mode: start recording on press
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
                            // Continuous mode ignores release events (toggle behavior)
                            if is_continuous_mode {
                                continue;
                            }
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

                                    // Track pending transcription with backpressure
                                    let accepted = tracker.add_pending_with_config(
                                        mark.sequence_id,
                                        next_chunk_id,
                                        bp.max_pending,
                                        bp.high_water_mark,
                                        bp.strategy,
                                    );
                                    if !accepted {
                                        warn!(
                                            "Final chunk rejected due to backpressure (seq {}.{})",
                                            mark.sequence_id, next_chunk_id
                                        );
                                        #[cfg(unix)]
                                        notify_backpressure(bp.notify);
                                    } else {
                                        // Submit final job only if accepted
                                        let job = TranscriptionJob {
                                            buffer,
                                            sequence_id: mark.sequence_id,
                                            chunk_id: next_chunk_id,
                                            is_final: true,
                                        };
                                        job_tx.send(job).await.map_err(|_| {
                                            error!("Transcription worker failed - channel closed");
                                            DaemonError::WorkerFailed
                                        })?;
                                    }
                                } else if next_chunk_id == 0 {
                                    // No chunks emitted and final chunk too short
                                    warn!("Recording too short, ignoring");
                                } else {
                                    debug!("Final chunk too short, but {} chunks already emitted", next_chunk_id);
                                }

                                // Flush any buffered results now that hotkey is released
                                for ready in tracker.take_ready() {
                                    process_and_output(
                                        ready,
                                        &chunk_separator,
                                        &vocabulary_manager,
                                        &text_corrector,
                                        &output_handler,
                                    ).await;
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
                            process_and_output(
                                ready,
                                &chunk_separator,
                                &vocabulary_manager,
                                &text_corrector,
                                &output_handler,
                            ).await;
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

                            // Track pending transcription with backpressure
                            let accepted = tracker.add_pending_with_config(
                                mark.sequence_id,
                                *next_chunk_id,
                                bp.max_pending,
                                bp.high_water_mark,
                                bp.strategy,
                            );
                            if !accepted {
                                warn!(
                                    "Streaming chunk rejected due to backpressure (seq {}.{})",
                                    mark.sequence_id, *next_chunk_id
                                );
                                #[cfg(unix)]
                                notify_backpressure(bp.notify);
                                // Skip submitting the job but still update state
                                *last_chunk_pos = current_pos;
                                *next_chunk_id += 1;
                            } else {
                                // Submit chunk job only if accepted
                                let job = TranscriptionJob {
                                    buffer,
                                    sequence_id: mark.sequence_id,
                                    chunk_id: *next_chunk_id,
                                    is_final: false,
                                };
                                job_tx.send(job).await.map_err(|_| {
                                    error!("Transcription worker failed - channel closed");
                                    DaemonError::WorkerFailed
                                })?;

                                // Update state for next chunk
                                *last_chunk_pos = current_pos;
                                *next_chunk_id += 1;
                            }
                        } else {
                            debug!("Chunk too short, skipping");
                        }
                    }
                }

                // Handle VAD timer tick (continuous dictation mode)
                _ = async {
                    if let Some(timer) = &mut vad_timer {
                        timer.tick().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    if let DaemonState::ContinuousRecording {
                        ref mark,
                        ref mut speech_start_pos,
                        ref mut last_vad_pos,
                        ref mut next_chunk_id,
                    } = self.state {
                        let current_pos = audio_recorder.current_position();

                        // Extract audio from last VAD position to current for VAD processing
                        if let Some(buffer) = audio_recorder.extract_chunk(*last_vad_pos, current_pos) {
                            let samples = &buffer.samples;

                            // Process through VAD
                            if let (Some(ref mut engine), Some(ref mut state)) = (&mut vad_engine, &mut vad_state) {
                                match engine.process(samples) {
                                    Ok(result) => {
                                        // Track speech start position
                                        if result.is_speech && speech_start_pos.is_none() {
                                            *speech_start_pos = Some(*last_vad_pos);
                                            debug!("VAD: Speech started at pos {} (prob: {:.2})", *last_vad_pos, result.probability);
                                        }

                                        // Update state and check for speech segment completion
                                        if let Some(segment) = state.update(&result, samples.len()) {
                                            // Speech segment detected - extract and transcribe
                                            let segment_start = speech_start_pos.take().unwrap_or(segment.start);
                                            let segment_end = current_pos;

                                            if let Some(segment_buffer) = audio_recorder.extract_chunk(segment_start, segment_end) {
                                                info!(
                                                    "🎤 VAD speech segment {:.2}s (seq {}.{}, prob: {:.2})",
                                                    segment_buffer.duration_secs(),
                                                    mark.sequence_id,
                                                    *next_chunk_id,
                                                    segment.avg_probability
                                                );

                                                // Track pending transcription with backpressure
                                                let accepted = tracker.add_pending_with_config(
                                                    mark.sequence_id,
                                                    *next_chunk_id,
                                                    bp.max_pending,
                                                    bp.high_water_mark,
                                                    bp.strategy,
                                                );
                                                if !accepted {
                                                    warn!(
                                                        "VAD segment rejected due to backpressure (seq {}.{})",
                                                        mark.sequence_id, *next_chunk_id
                                                    );
                                                    #[cfg(unix)]
                                                    notify_backpressure(bp.notify);
                                                } else {
                                                    // Submit transcription job
                                                    let job = TranscriptionJob {
                                                        buffer: segment_buffer,
                                                        sequence_id: mark.sequence_id,
                                                        chunk_id: *next_chunk_id,
                                                        is_final: false, // Continuous mode, more may come
                                                    };
                                                    job_tx.send(job).await.map_err(|_| {
                                                        error!("Transcription worker failed - channel closed");
                                                        DaemonError::WorkerFailed
                                                    })?;
                                                }
                                                *next_chunk_id += 1;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("VAD processing error: {}", e);
                                    }
                                }
                            }
                        }
                        *last_vad_pos = current_pos;
                    }
                }

                // Handle vocabulary reload timer
                _ = async {
                    if let Some(timer) = &mut vocab_reload_timer {
                        timer.tick().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    if let Some(ref vocab) = vocabulary_manager {
                        match vocab.check_reload().await {
                            Ok(true) => {
                                info!(
                                    "Vocabulary reloaded ({} rules)",
                                    vocab.rule_count().await
                                );
                            }
                            Ok(false) => {} // No changes
                            Err(e) => {
                                warn!("Failed to reload vocabulary: {}", e);
                            }
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
        drop(job_tx); // Signal worker to stop by closing the channel

        // Wait for worker thread to finish (with timeout)
        info!("Waiting for transcription worker to finish...");
        match worker_handle.join() {
            Ok(()) => info!("Transcription worker stopped cleanly"),
            Err(_) => warn!("Transcription worker thread panicked during shutdown"),
        }

        info!("Daemon stopped");
        Ok(())
    }
}

/// Set up Unix signal handlers for the daemon
///
/// This is called from run_loop to set up SIGTERM and SIGHUP handling.
/// Returns Result instead of panicking to allow graceful error handling.
#[cfg(unix)]
pub fn setup_signal_handlers(
) -> Result<(tokio::signal::unix::Signal, tokio::signal::unix::Signal), std::io::Error> {
    use tokio::signal::unix::{signal, SignalKind};

    let sigterm = signal(SignalKind::terminate())?;
    let sighup = signal(SignalKind::hangup())?;

    Ok((sigterm, sighup))
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
                #[cfg(unix)]
                {
                    if let Ok(pid) = pid_str.trim().parse::<i32>() {
                        use nix::sys::signal::kill;
                        use nix::unistd::Pid;
                        // kill with signal None (0) only checks if process exists
                        return kill(Pid::from_raw(pid), None).is_ok();
                    }
                }
                #[cfg(not(unix))]
                {
                    // On non-unix, we can only check if PID file exists and has valid content
                    if pid_str.trim().parse::<i32>().is_ok() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Write PID file atomically using O_CREAT | O_EXCL to prevent race conditions
fn write_pid() -> Result<(), DaemonError> {
    use std::io::Write;

    let path = pid_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Use create_new() which maps to O_CREAT | O_EXCL - fails if file exists
    // This prevents TOCTOU race between is_running() check and write
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                DaemonError::AlreadyRunning
            } else {
                DaemonError::Io(e)
            }
        })?;

    write!(file, "{}", std::process::id())?;
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
        #[cfg(windows)]
        {
            hide_console_window();
        }
    }

    write_pid()?;

    let mut daemon = Daemon::new(config)?;
    let result = daemon.run_loop(enable_tray).await;

    remove_pid()?;

    result
}

/// Hide the console window on Windows for background daemon mode
#[cfg(windows)]
fn hide_console_window() {
    use windows_sys::Win32::System::Console::{FreeConsole, GetConsoleWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    unsafe {
        let console_window = GetConsoleWindow();
        if !console_window.is_null() {
            // First hide the window, then detach from console
            ShowWindow(console_window, SW_HIDE);
            FreeConsole();
            info!("Console window hidden for background operation");
        }
    }
}

/// Perform Unix daemonization using nix crate
#[cfg(unix)]
fn daemonize_process() -> Result<(), DaemonError> {
    use nix::unistd::{chdir, dup2, fork, setsid, ForkResult};
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    // Get log directory for stdout/stderr redirection
    let log_dir = Config::data_dir().map_err(|e| DaemonError::DaemonizeFailed(e.to_string()))?;
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        warn!(
            "Failed to create log directory {}: {}",
            log_dir.display(),
            e
        );
    }

    // Create stdout/stderr files for daemon output
    let stdout_path = log_dir.join("daemon.out");
    let stderr_path = log_dir.join("daemon.err");

    let stdout_file = File::create(&stdout_path)
        .map_err(|e| DaemonError::DaemonizeFailed(format!("Cannot create stdout file: {}", e)))?;
    let stderr_file = File::create(&stderr_path)
        .map_err(|e| DaemonError::DaemonizeFailed(format!("Cannot create stderr file: {}", e)))?;

    // Print before forking so user sees the message
    println!("Daemonizing OpenHush (logs: {:?})...", log_dir);

    // First fork: create child process
    // SAFETY: fork() is safe when called before spawning threads.
    // We're in the startup path before any threads are created.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // Parent exits, child continues
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Continue in child
        }
        Err(e) => {
            return Err(DaemonError::DaemonizeFailed(format!(
                "First fork failed: {}",
                e
            )));
        }
    }

    // Create new session, become session leader
    setsid().map_err(|e| DaemonError::DaemonizeFailed(format!("setsid failed: {}", e)))?;

    // Second fork: ensure we can never acquire a controlling terminal
    // SAFETY: Same as above, we're still single-threaded at this point
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Continue in grandchild
        }
        Err(e) => {
            return Err(DaemonError::DaemonizeFailed(format!(
                "Second fork failed: {}",
                e
            )));
        }
    }

    // Change working directory to root
    chdir("/").map_err(|e| DaemonError::DaemonizeFailed(format!("chdir failed: {}", e)))?;

    // Redirect stdout/stderr to log files
    // Note: We don't redirect stdin as it's not needed for a daemon
    dup2(stdout_file.as_raw_fd(), 1)
        .map_err(|e| DaemonError::DaemonizeFailed(format!("dup2 stdout failed: {}", e)))?;
    dup2(stderr_file.as_raw_fd(), 2)
        .map_err(|e| DaemonError::DaemonizeFailed(format!("dup2 stderr failed: {}", e)))?;

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

    // Check if process is actually running
    #[cfg(unix)]
    {
        let pid: i32 = match pid_str.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                // Invalid PID file content, remove it
                warn!("Removing invalid PID file");
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Failed to remove invalid PID file: {}", e);
                }
                return Ok(());
            }
        };

        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        // kill with signal None (0) only checks if process exists
        if kill(Pid::from_raw(pid), None).is_err() {
            // Process not running, this is a stale PID file
            warn!(
                "Removing stale PID file (process {} no longer running)",
                pid
            );
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Failed to remove stale PID file: {}", e);
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-unix, just validate the PID file content
        if pid_str.trim().parse::<i32>().is_err() {
            warn!("Removing invalid PID file");
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Failed to remove invalid PID file: {}", e);
            }
        }
        // Can't check if process is running on non-unix without platform-specific APIs
    }

    Ok(())
}

/// Verify that a PID belongs to an openhush process
#[cfg(target_os = "linux")]
fn verify_openhush_process(pid: i32) -> bool {
    // Check /proc/<pid>/exe to verify it's openhush
    let exe_path = format!("/proc/{}/exe", pid);
    if let Ok(target) = std::fs::read_link(&exe_path) {
        if let Some(name) = target.file_name() {
            return name == "openhush";
        }
    }
    false
}

#[cfg(all(unix, not(target_os = "linux")))]
fn verify_openhush_process(_pid: i32) -> bool {
    // On non-Linux Unix, we can't easily verify the process name
    // Fall back to trusting the PID file
    true
}

/// Stop the daemon
pub async fn stop() -> Result<(), DaemonError> {
    // Try IPC first on macOS and Windows
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use crate::ipc::{IpcClient, IpcCommand};

        match IpcClient::connect() {
            Ok(mut client) => match client.send(IpcCommand::Stop) {
                Ok(response) => {
                    if response.ok {
                        info!("Stop command sent successfully via IPC");
                        return Ok(());
                    } else {
                        warn!("Stop command failed: {:?}", response.error);
                    }
                }
                Err(e) => {
                    warn!("Failed to send stop command via IPC: {}", e);
                }
            },
            Err(crate::ipc::IpcError::NotRunning) => {
                return Err(DaemonError::NotRunning);
            }
            Err(e) => {
                warn!("Failed to connect to daemon via IPC: {}", e);
            }
        }
    }

    // Linux uses SIGTERM (D-Bus is handled separately if available)
    #[cfg(target_os = "linux")]
    {
        if !is_running() {
            return Err(DaemonError::NotRunning);
        }

        let path = pid_file()?;
        if let Ok(pid_str) = std::fs::read_to_string(&path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Validate PID is in reasonable range
                if pid <= 0 {
                    warn!("Invalid PID {} in PID file, removing", pid);
                    if let Err(e) = std::fs::remove_file(&path) {
                        warn!("Failed to remove invalid PID file: {}", e);
                    }
                    return Err(DaemonError::NotRunning);
                }

                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                // Verify the process is actually openhush before sending signal
                if !verify_openhush_process(pid) {
                    warn!(
                        "PID {} is not an openhush process, removing stale PID file",
                        pid
                    );
                    if let Err(e) = std::fs::remove_file(&path) {
                        warn!("Failed to remove stale PID file: {}", e);
                    }
                    return Err(DaemonError::NotRunning);
                }

                // Send SIGTERM to gracefully stop the daemon
                if let Err(e) = kill(Pid::from_raw(pid), Signal::SIGTERM) {
                    warn!("Failed to send SIGTERM to daemon: {}", e);
                } else {
                    info!("Sent SIGTERM to daemon (PID: {})", pid);
                }
            }
        }
    }

    Ok(())
}

/// Check daemon status
pub async fn status() -> Result<(), DaemonError> {
    // Try IPC first on macOS and Windows for detailed status
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use crate::ipc::{IpcClient, IpcCommand};

        match IpcClient::connect() {
            Ok(mut client) => match client.send(IpcCommand::Status) {
                Ok(response) => {
                    if response.ok {
                        println!("OpenHush daemon is running");
                        if let Some(version) = response.version {
                            println!("  Version: {}", version);
                        }
                        if let Some(recording) = response.recording {
                            println!("  Recording: {}", if recording { "yes" } else { "no" });
                        }
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!("Failed to get status via IPC: {}", e);
                }
            },
            Err(crate::ipc::IpcError::NotRunning) => {
                println!("OpenHush daemon is not running");
                return Ok(());
            }
            Err(e) => {
                warn!("Failed to connect to daemon via IPC: {}", e);
            }
        }
    }

    // Fallback to PID file check
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
