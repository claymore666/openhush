//! Application state and logic for the TUI.

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
        Self {
            running: true,
            theme: Theme::terminal_default(),
            active_panel: ActivePanel::default(),
            recording_state: RecordingState::default(),
            recording_duration: 0.0,
            audio_level: 0.0,
            audio_history: vec![0.0; 32],
            current_transcription: String::new(),
            history: vec![
                TranscriptionEntry {
                    timestamp: "14:32:05".to_string(),
                    text: "Hello world, this is a test transcription".to_string(),
                    duration_secs: 2.3,
                },
                TranscriptionEntry {
                    timestamp: "14:31:42".to_string(),
                    text: "Previous dictation appears here".to_string(),
                    duration_secs: 1.8,
                },
                TranscriptionEntry {
                    timestamp: "14:30:18".to_string(),
                    text: "Older entries scroll down in the history panel".to_string(),
                    duration_secs: 3.1,
                },
            ],
            history_index: 0,
            model_name: "large-v3".to_string(),
            language: "auto".to_string(),
            vad_enabled: true,
            llm_enabled: true,
            llm_provider: "ollama".to_string(),
            show_help: false,
        }
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
        // Simulate audio level changes for demo
        if self.recording_state == RecordingState::Recording {
            self.recording_duration += 0.25;
            // Simulate audio level
            self.audio_level = (self.recording_duration * 3.0).sin().abs() * 0.7 + 0.1;
        } else {
            self.audio_level *= 0.9; // Decay when not recording
        }

        // Update audio history
        self.audio_history.remove(0);
        self.audio_history.push(self.audio_level);
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

    /// Stop recording and start processing.
    fn stop_recording(&mut self) {
        if self.recording_state == RecordingState::Recording {
            self.recording_state = RecordingState::Processing;
            // Simulate transcription result
            self.current_transcription =
                "This is a simulated transcription result from the recording.".to_string();
            // Add to history
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
