//! Application state and logic for the TUI.

use crate::ipc::{DaemonState, IpcEvent};
use crate::tui::daemon::{ConnectionState, DaemonClient};
use crate::tui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

/// Application result type.
pub type AppResult<T> = anyhow::Result<T>;

/// The active panel in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Status,
    Transcription,
    History,
}

/// Recording state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    #[default]
    Idle,
    Recording,
    Processing,
}

/// Main application state.
pub struct App {
    /// Is the application running?
    running: bool,
    /// Color theme
    pub theme: Theme,
    /// Daemon client for IPC communication
    pub daemon: DaemonClient,
    /// Currently active panel
    pub active_panel: ActivePanel,
    /// Recording state
    pub recording_state: RecordingState,
    /// Recording duration in seconds
    pub recording_duration: f32,
    /// Current audio level (0.0 to 1.0)
    pub audio_level: f32,
    /// Audio level history for visualization (last N samples)
    pub audio_history: Vec<f32>,
    /// Current transcription text
    pub current_transcription: String,
    /// Transcription history
    pub history: Vec<TranscriptionEntry>,
    /// Selected history index
    pub history_index: usize,
    /// Current model name
    pub model_name: String,
    /// Current language
    pub language: String,
    /// VAD enabled
    pub vad_enabled: bool,
    /// LLM correction enabled
    pub llm_enabled: bool,
    /// LLM provider name
    pub llm_provider: String,
    /// Show help overlay
    pub show_help: bool,
    /// Status message (for errors/info)
    pub status_message: Option<String>,
}

/// A transcription history entry.
#[derive(Debug, Clone)]
pub struct TranscriptionEntry {
    pub timestamp: String,
    pub text: String,
    #[allow(dead_code)] // Will be used for display later
    pub duration_secs: f32,
}

impl App {
    /// Create a new App instance.
    pub fn new() -> Self {
        // Don't connect here - let on_tick() handle it asynchronously
        // This ensures the UI appears immediately
        let daemon = DaemonClient::new();

        Self {
            running: true,
            theme: Theme::terminal_default(),
            daemon,
            active_panel: ActivePanel::default(),
            recording_state: RecordingState::default(),
            recording_duration: 0.0,
            audio_level: 0.0,
            audio_history: vec![0.0; 32],
            current_transcription: String::new(),
            history: Vec::new(),
            history_index: 0,
            model_name: "large-v3".to_string(),
            language: "auto".to_string(),
            vad_enabled: true,
            llm_enabled: true,
            llm_provider: "ollama".to_string(),
            show_help: false,
            status_message: None,
        }
    }

    /// Get the daemon connection state.
    #[allow(dead_code)]
    pub fn connection_state(&self) -> ConnectionState {
        self.daemon.state()
    }

    /// Check if the application is still running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Handle tick events (called periodically).
    pub fn on_tick(&mut self) {
        // Handle daemon reconnection
        self.daemon.handle_reconnect();

        // Process daemon events
        for event in self.daemon.poll_events() {
            self.handle_daemon_event(event);
        }

        // Update from daemon status if connected
        if self.daemon.is_connected() {
            if let Some(status) = self.daemon.last_status() {
                self.model_name = status.model.clone();
                self.recording_state = match status.state {
                    DaemonState::Idle => RecordingState::Idle,
                    DaemonState::Recording => RecordingState::Recording,
                    DaemonState::Processing => RecordingState::Processing,
                };
                if let Some(dur) = status.recording_duration {
                    self.recording_duration = dur as f32;
                }
            }
        } else {
            // Simulate audio level changes for demo when not connected
            if self.recording_state == RecordingState::Recording {
                self.recording_duration += 0.25;
                self.audio_level = (self.recording_duration * 3.0).sin().abs() * 0.7 + 0.1;
            } else {
                self.audio_level *= 0.9;
            }
        }

        // Update audio history
        self.audio_history.remove(0);
        self.audio_history.push(self.audio_level);
    }

    /// Handle a daemon event.
    fn handle_daemon_event(&mut self, event: IpcEvent) {
        match event {
            IpcEvent::AudioLevel {
                rms_db,
                peak_db: _,
                vad_active: _,
            } => {
                // Convert dB to linear (0.0 to 1.0)
                // rms_db is typically -60 to 0
                self.audio_level = ((rms_db + 60.0) / 60.0).clamp(0.0, 1.0);
            }
            IpcEvent::RecordingStarted {
                recording_id: _,
                timestamp: _,
            } => {
                self.recording_state = RecordingState::Recording;
                self.recording_duration = 0.0;
                self.current_transcription.clear();
            }
            IpcEvent::RecordingStopped {
                recording_id: _,
                duration_secs,
            } => {
                self.recording_duration = duration_secs as f32;
                self.recording_state = RecordingState::Processing;
            }
            IpcEvent::TranscriptionStarted { recording_id: _ } => {
                self.recording_state = RecordingState::Processing;
            }
            IpcEvent::TranscriptionComplete {
                id: _,
                recording_id: _,
                text,
                duration_secs,
                llm_corrected: _,
            } => {
                self.current_transcription = text.clone();
                self.history.insert(
                    0,
                    TranscriptionEntry {
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        text,
                        duration_secs: duration_secs as f32,
                    },
                );
                self.recording_state = RecordingState::Idle;
            }
            IpcEvent::StateChanged { state } => {
                self.recording_state = match state {
                    DaemonState::Idle => RecordingState::Idle,
                    DaemonState::Recording => RecordingState::Recording,
                    DaemonState::Processing => RecordingState::Processing,
                };
            }
            IpcEvent::Error { code: _, message } => {
                self.status_message = Some(message);
            }
            IpcEvent::Shutdown => {
                self.status_message = Some("Daemon shutting down".to_string());
            }
            _ => {}
        }
    }

    /// Handle key events.
    pub fn on_key(&mut self, key: KeyEvent) {
        // Global shortcuts
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                self.quit();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit();
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                }
            }
            _ => {}
        }

        // Don't process other keys if help is shown
        if self.show_help {
            return;
        }

        match key.code {
            // Panel navigation
            KeyCode::Tab => {
                self.active_panel = match self.active_panel {
                    ActivePanel::Status => ActivePanel::Transcription,
                    ActivePanel::Transcription => ActivePanel::History,
                    ActivePanel::History => ActivePanel::Status,
                };
            }
            KeyCode::BackTab => {
                self.active_panel = match self.active_panel {
                    ActivePanel::Status => ActivePanel::History,
                    ActivePanel::Transcription => ActivePanel::Status,
                    ActivePanel::History => ActivePanel::Transcription,
                };
            }

            // Recording control
            KeyCode::Char('r') => {
                self.toggle_recording();
            }
            KeyCode::Char('s') => {
                self.stop_recording();
            }

            // History navigation
            KeyCode::Up | KeyCode::Char('k') => {
                if self.active_panel == ActivePanel::History && self.history_index > 0 {
                    self.history_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.active_panel == ActivePanel::History
                    && self.history_index < self.history.len().saturating_sub(1)
                {
                    self.history_index += 1;
                }
            }

            // Quick actions (from any panel)
            KeyCode::Char('h') => {
                self.active_panel = ActivePanel::History;
            }
            KeyCode::Char('m') => {
                // TODO: Open model manager
            }
            KeyCode::Char('c') => {
                // TODO: Open config
            }
            KeyCode::Char('i') => {
                // TODO: Open input selector
            }
            KeyCode::Char('o') => {
                // TODO: Open output config
            }

            _ => {}
        }
    }

    /// Handle mouse events.
    pub fn on_mouse(&mut self, _mouse: MouseEvent) {
        // TODO: Handle mouse clicks on panels
    }

    /// Toggle recording state.
    fn toggle_recording(&mut self) {
        if self.daemon.is_connected() {
            // Use daemon to toggle
            if let Err(e) = self.daemon.toggle_recording() {
                self.status_message = Some(format!("Recording error: {}", e));
            }
        } else {
            // Local simulation when daemon not connected
            match self.recording_state {
                RecordingState::Idle => {
                    self.recording_state = RecordingState::Recording;
                    self.recording_duration = 0.0;
                    self.current_transcription.clear();
                }
                RecordingState::Recording => {
                    self.stop_recording();
                }
                RecordingState::Processing => {
                    // Can't toggle while processing
                }
            }
        }
    }

    /// Stop recording and start processing.
    fn stop_recording(&mut self) {
        if self.daemon.is_connected() {
            // Use daemon to stop
            if let Err(e) = self.daemon.stop_recording() {
                self.status_message = Some(format!("Stop recording error: {}", e));
            }
        } else if self.recording_state == RecordingState::Recording {
            // Local simulation
            self.recording_state = RecordingState::Processing;
            self.current_transcription =
                "This is a simulated transcription (daemon not connected).".to_string();
            self.history.insert(
                0,
                TranscriptionEntry {
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                    text: self.current_transcription.clone(),
                    duration_secs: self.recording_duration,
                },
            );
            self.recording_state = RecordingState::Idle;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
