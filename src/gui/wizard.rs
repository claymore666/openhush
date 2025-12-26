//! First-run onboarding wizard using egui.
//!
//! Guides new users through initial setup:
//! 1. Welcome - Introduction and privacy notice
//! 2. Microphone - Select and test audio input
//! 3. Model - Choose and download Whisper model
//! 4. Hotkey - Configure trigger key
//! 5. Output - Choose clipboard/paste behavior
//! 6. Ollama - Optional LLM correction setup
//! 7. Complete - Summary and quick test

#![allow(dead_code)]

use crate::config::{Config, TranscriptionPreset};
use crate::input::audio::AudioRecorder;
use eframe::egui;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info};

/// Wizard step enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Welcome,
    Microphone,
    Model,
    Hotkey,
    Output,
    Ollama,
    Complete,
}

impl WizardStep {
    fn title(&self) -> &'static str {
        match self {
            WizardStep::Welcome => "Welcome to OpenHush",
            WizardStep::Microphone => "Microphone Setup",
            WizardStep::Model => "Model Selection",
            WizardStep::Hotkey => "Hotkey Configuration",
            WizardStep::Output => "Output Settings",
            WizardStep::Ollama => "LLM Correction (Optional)",
            WizardStep::Complete => "Setup Complete",
        }
    }

    fn index(&self) -> usize {
        match self {
            WizardStep::Welcome => 0,
            WizardStep::Microphone => 1,
            WizardStep::Model => 2,
            WizardStep::Hotkey => 3,
            WizardStep::Output => 4,
            WizardStep::Ollama => 5,
            WizardStep::Complete => 6,
        }
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(WizardStep::Welcome),
            1 => Some(WizardStep::Microphone),
            2 => Some(WizardStep::Model),
            3 => Some(WizardStep::Hotkey),
            4 => Some(WizardStep::Output),
            5 => Some(WizardStep::Ollama),
            6 => Some(WizardStep::Complete),
            _ => None,
        }
    }

    fn total_steps() -> usize {
        7
    }
}

/// Model download state
#[derive(Clone)]
enum DownloadState {
    NotStarted,
    Downloading {
        progress: f32,
        downloaded: u64,
        total: u64,
    },
    Completed,
    Failed(String),
}

/// Microphone test state
struct MicTestState {
    audio_recorder: Option<AudioRecorder>,
    level_db: f32,
    is_testing: bool,
}

impl Default for MicTestState {
    fn default() -> Self {
        Self {
            audio_recorder: None,
            level_db: f32::NEG_INFINITY,
            is_testing: false,
        }
    }
}

/// The onboarding wizard application
struct OnboardingWizard {
    current_step: WizardStep,
    config: Config,

    // Microphone state
    available_mics: Vec<String>,
    selected_mic: usize,
    mic_test: MicTestState,

    // Model state
    selected_model: String,
    download_state: Arc<Mutex<DownloadState>>,
    download_receiver: Option<mpsc::Receiver<DownloadState>>,

    // Hotkey state
    hotkey_listening: bool,
    hotkey_detected: Option<String>,

    // Status
    status_message: Option<String>,
    wizard_completed: bool,
}

impl OnboardingWizard {
    fn new() -> Self {
        let config = Config::default();
        let available_mics = AudioRecorder::list_devices();

        Self {
            current_step: WizardStep::Welcome,
            config,
            available_mics,
            selected_mic: 0,
            mic_test: MicTestState::default(),
            selected_model: "small".to_string(),
            download_state: Arc::new(Mutex::new(DownloadState::NotStarted)),
            download_receiver: None,
            hotkey_listening: false,
            hotkey_detected: None,
            status_message: None,
            wizard_completed: false,
        }
    }

    fn next_step(&mut self) {
        if let Some(next) = WizardStep::from_index(self.current_step.index() + 1) {
            self.current_step = next;
        }
    }

    fn prev_step(&mut self) {
        if self.current_step.index() > 0 {
            if let Some(prev) = WizardStep::from_index(self.current_step.index() - 1) {
                self.current_step = prev;
            }
        }
    }

    fn can_proceed(&self) -> bool {
        match self.current_step {
            WizardStep::Welcome => true,
            WizardStep::Microphone => !self.available_mics.is_empty(),
            WizardStep::Model => {
                matches!(
                    *self.download_state.lock().unwrap(),
                    DownloadState::Completed | DownloadState::NotStarted
                )
            }
            WizardStep::Hotkey => !self.config.hotkey.key.is_empty(),
            WizardStep::Output => true,
            WizardStep::Ollama => true,
            WizardStep::Complete => true,
        }
    }

    fn finish_wizard(&mut self) -> anyhow::Result<()> {
        // Apply selected model based on preset
        self.config.transcription.preset = match self.selected_model.as_str() {
            "tiny" | "base" | "small" => TranscriptionPreset::Instant,
            "medium" => TranscriptionPreset::Balanced,
            "large-v3" => TranscriptionPreset::Quality,
            _ => TranscriptionPreset::Custom,
        };
        self.config.transcription.model = self.selected_model.clone();

        // Save configuration
        self.config.save()?;
        info!("Wizard completed, configuration saved");
        self.wizard_completed = true;
        Ok(())
    }

    // ============================================================================
    // Step renderers
    // ============================================================================

    fn show_welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading("Welcome to OpenHush!");
            ui.add_space(20.0);
        });

        ui.label("OpenHush is a voice-to-text tool that lets you dictate text anywhere.");
        ui.add_space(10.0);

        ui.label("This wizard will help you set up:");
        ui.add_space(5.0);
        ui.indent("setup_list", |ui| {
            ui.label("• Select your microphone");
            ui.label("• Download a speech recognition model");
            ui.label("• Configure your trigger hotkey");
            ui.label("• Set up output preferences");
        });

        ui.add_space(20.0);

        ui.group(|ui| {
            ui.label("🔒 Privacy Notice");
            ui.add_space(5.0);
            ui.label("All speech recognition happens locally on your device.");
            ui.label("No audio data is sent to the cloud.");
        });

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            if ui.button("Skip Wizard").clicked() {
                // Save default config and close
                if let Err(e) = Config::default().save() {
                    error!("Failed to save default config: {}", e);
                }
                self.wizard_completed = true;
            }
        });
    }

    fn show_microphone(&mut self, ui: &mut egui::Ui) {
        ui.heading("Select Your Microphone");
        ui.add_space(10.0);

        if self.available_mics.is_empty() {
            ui.colored_label(egui::Color32::RED, "⚠ No microphones detected!");
            ui.label("Please connect a microphone and restart the wizard.");
            return;
        }

        ui.label("Available microphones:");
        ui.add_space(5.0);

        egui::ComboBox::from_id_salt("mic_select")
            .selected_text(
                self.available_mics
                    .get(self.selected_mic)
                    .cloned()
                    .unwrap_or_else(|| "Select...".to_string()),
            )
            .show_ui(ui, |ui| {
                for (i, mic) in self.available_mics.iter().enumerate() {
                    ui.selectable_value(&mut self.selected_mic, i, mic);
                }
            });

        ui.add_space(20.0);

        // Microphone test
        ui.group(|ui| {
            ui.label("🎤 Test Microphone");
            ui.add_space(10.0);

            if self.mic_test.is_testing {
                // Show audio level bar
                let level = self.mic_test.level_db;
                let normalized = ((level + 60.0) / 60.0).clamp(0.0, 1.0);

                let color = if normalized > 0.8 {
                    egui::Color32::RED
                } else if normalized > 0.5 {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::GREEN
                };

                ui.horizontal(|ui| {
                    ui.label("Level:");
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(200.0, 20.0), egui::Sense::hover());

                    ui.painter()
                        .rect_filled(rect, 4.0, egui::Color32::DARK_GRAY);

                    let filled_width = rect.width() * normalized;
                    let filled_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(filled_width, rect.height()),
                    );
                    ui.painter().rect_filled(filled_rect, 4.0, color);
                });

                ui.add_space(5.0);

                if ui.button("Stop Test").clicked() {
                    self.mic_test.is_testing = false;
                    self.mic_test.audio_recorder = None;
                }
            } else if ui.button("Start Test").clicked() {
                self.start_mic_test();
            }
        });
    }

    fn start_mic_test(&mut self) {
        match AudioRecorder::new_always_on(2.0, Default::default()) {
            Ok(recorder) => {
                self.mic_test.audio_recorder = Some(recorder);
                self.mic_test.is_testing = true;
                self.mic_test.level_db = f32::NEG_INFINITY;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to start mic test: {}", e));
            }
        }
    }

    fn show_model(&mut self, ui: &mut egui::Ui) {
        ui.heading("Choose Speech Recognition Model");
        ui.add_space(10.0);

        ui.label("Larger models are more accurate but slower:");
        ui.add_space(10.0);

        let models = [
            ("tiny", "Tiny (~75 MB)", "Fastest, basic accuracy"),
            ("base", "Base (~145 MB)", "Fast, good accuracy"),
            (
                "small",
                "Small (~465 MB)",
                "Balanced speed/accuracy (Recommended)",
            ),
            ("medium", "Medium (~1.5 GB)", "High accuracy, slower"),
            ("large-v3", "Large V3 (~3 GB)", "Best accuracy, slowest"),
        ];

        for (id, name, desc) in models {
            let is_selected = self.selected_model == id;
            ui.horizontal(|ui| {
                if ui.radio(is_selected, name).clicked() {
                    self.selected_model = id.to_string();
                }
                ui.label(format!(" - {}", desc));
            });
        }

        ui.add_space(20.0);

        // Download section
        let state = self.download_state.lock().unwrap().clone();
        match state {
            DownloadState::NotStarted => {
                if ui.button("Download Model").clicked() {
                    self.start_model_download();
                }
            }
            DownloadState::Downloading {
                progress,
                downloaded,
                total,
            } => {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Downloading: {:.1}% ({} / {})",
                        progress * 100.0,
                        format_bytes(downloaded),
                        format_bytes(total)
                    ));
                });
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            }
            DownloadState::Completed => {
                ui.colored_label(egui::Color32::GREEN, "✓ Model downloaded successfully!");
            }
            DownloadState::Failed(err) => {
                ui.colored_label(egui::Color32::RED, format!("✗ Download failed: {}", err));
                if ui.button("Retry").clicked() {
                    *self.download_state.lock().unwrap() = DownloadState::NotStarted;
                }
            }
        }

        // Check for updates from download thread
        if let Some(ref rx) = self.download_receiver {
            while let Ok(state) = rx.try_recv() {
                *self.download_state.lock().unwrap() = state;
            }
        }
    }

    fn start_model_download(&mut self) {
        let model_name = self.selected_model.clone();
        let (tx, rx) = mpsc::channel();
        self.download_receiver = Some(rx);

        *self.download_state.lock().unwrap() = DownloadState::Downloading {
            progress: 0.0,
            downloaded: 0,
            total: 0,
        };

        thread::spawn(move || {
            use crate::engine::whisper::{download_model, WhisperModel};

            let model: WhisperModel = match model_name.parse() {
                Ok(m) => m,
                Err(()) => {
                    let _ = tx.send(DownloadState::Failed(format!(
                        "Unknown model '{}'",
                        model_name
                    )));
                    return;
                }
            };

            // Create a tokio runtime for the async download
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(DownloadState::Failed(format!(
                        "Failed to create runtime: {}",
                        e
                    )));
                    return;
                }
            };

            let tx_progress = tx.clone();
            let result = rt.block_on(download_model(model, move |downloaded, total| {
                let progress = if total > 0 {
                    downloaded as f32 / total as f32
                } else {
                    0.0
                };
                let _ = tx_progress.send(DownloadState::Downloading {
                    progress,
                    downloaded,
                    total,
                });
            }));

            match result {
                Ok(_) => {
                    let _ = tx.send(DownloadState::Completed);
                }
                Err(e) => {
                    let _ = tx.send(DownloadState::Failed(e.to_string()));
                }
            }
        });
    }

    fn show_hotkey(&mut self, ui: &mut egui::Ui) {
        ui.heading("Configure Trigger Hotkey");
        ui.add_space(10.0);

        ui.label("This key will start/stop voice recording.");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Hotkey:");
            ui.text_edit_singleline(&mut self.config.hotkey.key);
        });

        ui.add_space(5.0);
        ui.label("Common choices: ControlRight, F12, AltRight, ScrollLock");

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label("Mode:");
            let modes = ["push_to_talk", "toggle"];
            egui::ComboBox::from_id_salt("hotkey_mode")
                .selected_text(&self.config.hotkey.mode)
                .show_ui(ui, |ui| {
                    for mode in modes {
                        ui.selectable_value(&mut self.config.hotkey.mode, mode.to_string(), mode);
                    }
                });
        });

        ui.add_space(5.0);
        ui.label("Push-to-talk: Hold key while speaking");
        ui.label("Toggle: Press to start, press again to stop");
    }

    fn show_output(&mut self, ui: &mut egui::Ui) {
        ui.heading("Output Settings");
        ui.add_space(10.0);

        ui.label("How should OpenHush output transcribed text?");
        ui.add_space(10.0);

        ui.checkbox(&mut self.config.output.clipboard, "Copy to Clipboard");
        ui.label("  The transcribed text will be copied to your clipboard.");

        ui.add_space(10.0);

        ui.checkbox(&mut self.config.output.paste, "Paste at Cursor");
        ui.label("  The text will be automatically typed where your cursor is.");

        ui.add_space(20.0);

        ui.group(|ui| {
            ui.label("💡 Tip");
            ui.label("Most users enable both options for maximum flexibility.");
        });

        ui.add_space(20.0);

        ui.heading("Feedback");
        ui.add_space(10.0);

        ui.checkbox(
            &mut self.config.feedback.audio,
            "Audio feedback (beep sounds)",
        );
        ui.checkbox(&mut self.config.feedback.visual, "Visual notifications");
    }

    fn show_ollama(&mut self, ui: &mut egui::Ui) {
        ui.heading("LLM Correction (Optional)");
        ui.add_space(10.0);

        ui.label("OpenHush can use Ollama to improve transcription quality.");
        ui.label("This is optional and requires Ollama to be installed separately.");
        ui.add_space(10.0);

        ui.checkbox(&mut self.config.correction.enabled, "Enable LLM Correction");

        ui.add_space(10.0);

        ui.add_enabled_ui(self.config.correction.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Ollama URL:");
                ui.text_edit_singleline(&mut self.config.correction.ollama_url);
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Model:");
                ui.text_edit_singleline(&mut self.config.correction.ollama_model);
            });

            ui.add_space(10.0);
            ui.label("Recommended models: llama3.2:1b, phi3:mini, gemma2:2b");
        });

        ui.add_space(20.0);

        if !self.config.correction.enabled {
            ui.label("You can enable this later in Preferences.");
        }
    }

    fn show_complete(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading("🎉 Setup Complete!");
            ui.add_space(20.0);
        });

        ui.label("Your configuration:");
        ui.add_space(10.0);

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Hotkey:");
                ui.strong(&self.config.hotkey.key);
            });
            ui.horizontal(|ui| {
                ui.label("Model:");
                ui.strong(&self.selected_model);
            });
            ui.horizontal(|ui| {
                ui.label("Output:");
                let output = match (self.config.output.clipboard, self.config.output.paste) {
                    (true, true) => "Clipboard + Paste",
                    (true, false) => "Clipboard only",
                    (false, true) => "Paste only",
                    (false, false) => "None",
                };
                ui.strong(output);
            });
            ui.horizontal(|ui| {
                ui.label("LLM Correction:");
                ui.strong(if self.config.correction.enabled {
                    "Enabled"
                } else {
                    "Disabled"
                });
            });
        });

        ui.add_space(20.0);

        ui.label("You can start OpenHush from the terminal:");
        ui.add_space(5.0);
        ui.code("openhush start");

        ui.add_space(10.0);

        ui.label("Or change settings anytime with:");
        ui.add_space(5.0);
        ui.code("openhush preferences");
    }

    // ============================================================================
    // Progress indicator
    // ============================================================================

    fn show_progress(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for i in 0..WizardStep::total_steps() {
                let is_current = i == self.current_step.index();
                let is_complete = i < self.current_step.index();

                let color = if is_current {
                    egui::Color32::from_rgb(0, 120, 215)
                } else if is_complete {
                    egui::Color32::from_rgb(0, 180, 0)
                } else {
                    egui::Color32::GRAY
                };

                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());

                ui.painter().circle_filled(rect.center(), 10.0, color);

                if is_complete {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "✓",
                        egui::FontId::default(),
                        egui::Color32::WHITE,
                    );
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}", i + 1),
                        egui::FontId::default(),
                        egui::Color32::WHITE,
                    );
                }

                if i < WizardStep::total_steps() - 1 {
                    let line_color = if is_complete {
                        egui::Color32::from_rgb(0, 180, 0)
                    } else {
                        egui::Color32::GRAY
                    };
                    ui.painter().line_segment(
                        [
                            rect.right_center() + egui::vec2(2.0, 0.0),
                            rect.right_center() + egui::vec2(20.0, 0.0),
                        ],
                        egui::Stroke::new(2.0, line_color),
                    );
                    ui.add_space(22.0);
                }
            }
        });
    }
}

impl eframe::App for OnboardingWizard {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Close window if wizard completed
        if self.wizard_completed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Request repaint for animations (mic level, download progress)
        if self.mic_test.is_testing
            || matches!(
                *self.download_state.lock().unwrap(),
                DownloadState::Downloading { .. }
            )
        {
            ctx.request_repaint();
        }

        // Top panel with progress
        egui::TopBottomPanel::top("progress").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                self.show_progress(ui);
            });
            ui.add_space(10.0);
            ui.separator();
        });

        // Bottom panel with navigation
        egui::TopBottomPanel::bottom("navigation").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                // Back button
                ui.add_enabled_ui(self.current_step.index() > 0, |ui| {
                    if ui.button("← Back").clicked() {
                        self.prev_step();
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Next/Finish button
                    if self.current_step == WizardStep::Complete {
                        if ui.button("Finish").clicked() {
                            if let Err(e) = self.finish_wizard() {
                                self.status_message = Some(format!("Failed to save: {}", e));
                            }
                        }
                    } else {
                        ui.add_enabled_ui(self.can_proceed(), |ui| {
                            if ui.button("Next →").clicked() {
                                self.next_step();
                            }
                        });
                    }

                    // Status message
                    if let Some(ref msg) = self.status_message {
                        ui.colored_label(egui::Color32::RED, msg);
                    }
                });
            });
            ui.add_space(10.0);
        });

        // Central panel with current step content
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(10.0);

            // Step title
            ui.heading(self.current_step.title());
            ui.separator();
            ui.add_space(10.0);

            // Step content
            egui::ScrollArea::vertical().show(ui, |ui| match self.current_step {
                WizardStep::Welcome => self.show_welcome(ui),
                WizardStep::Microphone => self.show_microphone(ui),
                WizardStep::Model => self.show_model(ui),
                WizardStep::Hotkey => self.show_hotkey(ui),
                WizardStep::Output => self.show_output(ui),
                WizardStep::Ollama => self.show_ollama(ui),
                WizardStep::Complete => self.show_complete(ui),
            });
        });
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Check if this is the first run (no config file exists)
pub fn is_first_run() -> bool {
    match Config::config_path() {
        Ok(path) => !path.exists(),
        Err(_) => true,
    }
}

/// Run the onboarding wizard
pub fn run_wizard() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 500.0])
            .with_min_inner_size([500.0, 400.0])
            .with_title("OpenHush Setup"),
        ..Default::default()
    };

    eframe::run_native(
        "OpenHush Setup",
        options,
        Box::new(|_cc| Ok(Box::new(OnboardingWizard::new()))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run wizard: {}", e))
}

/// Run wizard if first run, otherwise do nothing
pub fn run_if_first_run() -> anyhow::Result<bool> {
    if is_first_run() {
        debug!("First run detected, launching setup wizard");
        run_wizard()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_step_from_index() {
        assert_eq!(WizardStep::from_index(0), Some(WizardStep::Welcome));
        assert_eq!(WizardStep::from_index(6), Some(WizardStep::Complete));
        assert_eq!(WizardStep::from_index(7), None);
    }

    #[test]
    fn test_wizard_step_index_roundtrip() {
        for i in 0..WizardStep::total_steps() {
            let step = WizardStep::from_index(i).unwrap();
            assert_eq!(step.index(), i);
        }
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_download_state_clone() {
        let state = DownloadState::Downloading {
            progress: 0.5,
            downloaded: 100,
            total: 200,
        };
        let cloned = state.clone();
        if let DownloadState::Downloading { progress, .. } = cloned {
            assert!((progress - 0.5).abs() < f32::EPSILON);
        } else {
            panic!("Expected Downloading state");
        }
    }
}
