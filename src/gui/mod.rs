//! GUI components using egui.
//!
//! - Preferences window for configuration
//! - First-run onboarding wizard

mod wizard;

pub use wizard::{is_first_run, run_wizard};

use crate::config::{Config, Theme};
use eframe::egui;
use tracing::{info, warn};

/// Run the preferences GUI as a standalone window
pub fn run_preferences() -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();
    let is_dark = config.appearance.theme.is_dark();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 600.0])
            .with_min_inner_size([400.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "OpenHush Preferences",
        options,
        Box::new(move |cc| {
            // Apply theme based on config
            apply_theme(&cc.egui_ctx, is_dark);
            Ok(Box::new(PreferencesApp::with_config(config)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run preferences: {}", e))
}

/// Apply light or dark theme to egui context
fn apply_theme(ctx: &egui::Context, is_dark: bool) {
    if is_dark {
        ctx.set_visuals(egui::Visuals::dark());
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
}

/// Spawn preferences window as a separate process (for use from daemon)
pub fn spawn_preferences() {
    // Launch a new process because GUI frameworks require the main thread
    let exe = std::env::current_exe().unwrap_or_else(|_| "openhush".into());
    match std::process::Command::new(exe).arg("preferences").spawn() {
        Ok(_) => tracing::info!("Preferences window spawned"),
        Err(e) => tracing::error!("Failed to spawn preferences: {}", e),
    }
}

struct PreferencesApp {
    config: Config,
    active_tab: Tab,
    unsaved_changes: bool,
    status_message: Option<(String, std::time::Instant)>,
    /// Temporary string for channel selection input
    channels_input: String,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Hotkey,
    Transcription,
    Audio,
    Output,
    Appearance,
    Advanced,
}

impl PreferencesApp {
    #[allow(dead_code)]
    fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        Self::with_config(config)
    }

    fn with_config(config: Config) -> Self {
        use crate::config::ChannelSelection;
        let channels_input = match &config.audio.channels {
            ChannelSelection::All => "all".to_string(),
            ChannelSelection::Select(chs) => chs
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        };
        Self {
            config,
            active_tab: Tab::Hotkey,
            unsaved_changes: false,
            status_message: None,
            channels_input,
        }
    }

    fn save_config(&mut self) {
        match self.config.save() {
            Ok(()) => {
                info!("Configuration saved");
                self.unsaved_changes = false;
                self.status_message = Some((
                    "Configuration saved!".to_string(),
                    std::time::Instant::now(),
                ));
            }
            Err(e) => {
                self.status_message =
                    Some((format!("Failed to save: {}", e), std::time::Instant::now()));
            }
        }
    }

    fn show_hotkey_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Hotkey Settings");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Trigger Key:");
            if ui
                .text_edit_singleline(&mut self.config.hotkey.key)
                .changed()
            {
                self.unsaved_changes = true;
            }
        });

        ui.add_space(5.0);
        ui.label("Examples: ControlRight, F12, AltRight");

        ui.add_space(15.0);

        ui.horizontal(|ui| {
            ui.label("Mode:");
            let modes = ["push_to_talk", "toggle"];
            egui::ComboBox::from_id_salt("hotkey_mode")
                .selected_text(&self.config.hotkey.mode)
                .show_ui(ui, |ui| {
                    for mode in modes {
                        if ui
                            .selectable_value(&mut self.config.hotkey.mode, mode.to_string(), mode)
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    }
                });
        });
    }

    fn show_transcription_tab(&mut self, ui: &mut egui::Ui) {
        use crate::config::TranscriptionPreset;

        ui.heading("Transcription Settings");
        ui.add_space(10.0);

        // Preset dropdown
        ui.horizontal(|ui| {
            ui.label("Preset:");
            let preset_text = match self.config.transcription.preset {
                TranscriptionPreset::Instant => "Instant (small)",
                TranscriptionPreset::Balanced => "Balanced (medium)",
                TranscriptionPreset::Quality => "Quality (large-v3)",
                TranscriptionPreset::Custom => "Custom",
            };
            egui::ComboBox::from_id_salt("preset")
                .selected_text(preset_text)
                .show_ui(ui, |ui| {
                    for (preset, label) in [
                        (TranscriptionPreset::Instant, "Instant (small)"),
                        (TranscriptionPreset::Balanced, "Balanced (medium)"),
                        (TranscriptionPreset::Quality, "Quality (large-v3)"),
                        (TranscriptionPreset::Custom, "Custom"),
                    ] {
                        if ui
                            .selectable_value(&mut self.config.transcription.preset, preset, label)
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    }
                });
        });

        ui.add_space(5.0);

        // Model dropdown (only enabled for Custom preset)
        let is_custom = self.config.transcription.preset == TranscriptionPreset::Custom;
        let effective_model = self.config.transcription.effective_model().to_string();
        ui.horizontal(|ui| {
            ui.label("Model:");
            let models = ["tiny", "base", "small", "medium", "large-v3"];
            ui.add_enabled_ui(is_custom, |ui| {
                egui::ComboBox::from_id_salt("model")
                    .selected_text(&effective_model)
                    .show_ui(ui, |ui| {
                        for model in models {
                            if ui
                                .selectable_value(
                                    &mut self.config.transcription.model,
                                    model.to_string(),
                                    model,
                                )
                                .changed()
                            {
                                self.unsaved_changes = true;
                            }
                        }
                    });
            });
            if !is_custom {
                ui.label("(set by preset)");
            }
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Language:");
            if ui
                .text_edit_singleline(&mut self.config.transcription.language)
                .changed()
            {
                self.unsaved_changes = true;
            }
        });
        ui.label("Use 'auto' for auto-detection, or ISO code (en, de, es, etc.)");

        ui.add_space(10.0);

        if ui
            .checkbox(
                &mut self.config.transcription.translate,
                "Translate to English",
            )
            .changed()
        {
            self.unsaved_changes = true;
        }

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Device:");
            let devices = ["cuda", "cpu"];
            egui::ComboBox::from_id_salt("device")
                .selected_text(&self.config.transcription.device)
                .show_ui(ui, |ui| {
                    for device in devices {
                        if ui
                            .selectable_value(
                                &mut self.config.transcription.device,
                                device.to_string(),
                                device,
                            )
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    }
                });
        });
    }

    fn show_audio_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Audio Preprocessing");
        ui.add_space(10.0);

        if ui
            .checkbox(
                &mut self.config.audio.preprocessing,
                "Enable Audio Preprocessing",
            )
            .changed()
        {
            self.unsaved_changes = true;
        }

        ui.add_space(10.0);

        ui.add_enabled_ui(self.config.audio.preprocessing, |ui| {
            ui.group(|ui| {
                // Normalization
                ui.collapsing("RMS Normalization", |ui| {
                    if ui
                        .checkbox(&mut self.config.audio.normalization.enabled, "Enabled")
                        .changed()
                    {
                        self.unsaved_changes = true;
                    }

                    ui.horizontal(|ui| {
                        ui.label("Target Level:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.config.audio.normalization.target_db,
                                    -40.0..=-6.0,
                                )
                                .suffix(" dB"),
                            )
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    });
                });

                ui.add_space(5.0);

                // Compression
                ui.collapsing("Dynamic Compression", |ui| {
                    if ui
                        .checkbox(&mut self.config.audio.compression.enabled, "Enabled")
                        .changed()
                    {
                        self.unsaved_changes = true;
                    }

                    ui.horizontal(|ui| {
                        ui.label("Threshold:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.config.audio.compression.threshold_db,
                                    -40.0..=-6.0,
                                )
                                .suffix(" dB"),
                            )
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Ratio:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.config.audio.compression.ratio,
                                    1.0..=20.0,
                                )
                                .suffix(":1"),
                            )
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Makeup Gain:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.config.audio.compression.makeup_gain_db,
                                    0.0..=24.0,
                                )
                                .suffix(" dB"),
                            )
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    });
                });

                ui.add_space(5.0);

                // Limiter
                ui.collapsing("Limiter", |ui| {
                    if ui
                        .checkbox(&mut self.config.audio.limiter.enabled, "Enabled")
                        .changed()
                    {
                        self.unsaved_changes = true;
                    }

                    ui.horizontal(|ui| {
                        ui.label("Ceiling:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.config.audio.limiter.ceiling_db,
                                    -6.0..=0.0,
                                )
                                .suffix(" dB"),
                            )
                            .changed()
                        {
                            self.unsaved_changes = true;
                        }
                    });
                });
            });
        });
    }

    fn show_output_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Output Settings");
        ui.add_space(10.0);

        if ui
            .checkbox(&mut self.config.output.clipboard, "Copy to Clipboard")
            .changed()
        {
            self.unsaved_changes = true;
        }

        if ui
            .checkbox(&mut self.config.output.paste, "Paste at Cursor")
            .changed()
        {
            self.unsaved_changes = true;
        }

        ui.add_space(15.0);

        ui.heading("Feedback");
        ui.add_space(10.0);

        if ui
            .checkbox(&mut self.config.feedback.audio, "Audio Feedback (beep)")
            .changed()
        {
            self.unsaved_changes = true;
        }

        if ui
            .checkbox(&mut self.config.feedback.visual, "Visual Notifications")
            .changed()
        {
            self.unsaved_changes = true;
        }
    }

    fn show_appearance_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Appearance");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Theme:");
            let theme_text = self.config.appearance.theme.display_name();
            egui::ComboBox::from_id_salt("theme")
                .selected_text(theme_text)
                .show_ui(ui, |ui| {
                    for (theme, label) in [
                        (Theme::Auto, "System"),
                        (Theme::Light, "Light"),
                        (Theme::Dark, "Dark"),
                    ] {
                        if ui
                            .selectable_value(&mut self.config.appearance.theme, theme, label)
                            .changed()
                        {
                            self.unsaved_changes = true;
                            // Apply theme immediately for preview
                            apply_theme(ctx, self.config.appearance.theme.is_dark());
                        }
                    }
                });
        });

        ui.add_space(5.0);
        ui.label("When set to 'System', the theme follows your desktop's dark/light mode setting.");

        ui.add_space(20.0);

        // Show current effective theme
        let effective = if self.config.appearance.theme.is_dark() {
            "Dark"
        } else {
            "Light"
        };
        ui.horizontal(|ui| {
            ui.label("Current theme:");
            ui.strong(effective);
        });
    }

    fn show_advanced_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Advanced Settings");
        ui.add_space(10.0);

        // LLM Correction
        ui.group(|ui| {
            ui.label("LLM Correction (Ollama)");
            ui.add_space(5.0);

            if ui
                .checkbox(&mut self.config.correction.enabled, "Enable LLM Correction")
                .changed()
            {
                self.unsaved_changes = true;
            }

            ui.add_enabled_ui(self.config.correction.enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ollama URL:");
                    if ui
                        .text_edit_singleline(&mut self.config.correction.ollama_url)
                        .changed()
                    {
                        self.unsaved_changes = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Model:");
                    if ui
                        .text_edit_singleline(&mut self.config.correction.ollama_model)
                        .changed()
                    {
                        self.unsaved_changes = true;
                    }
                });
            });
        });

        ui.add_space(15.0);

        // GPU Settings
        ui.group(|ui| {
            ui.label("GPU Settings");
            ui.add_space(5.0);

            if ui
                .checkbox(&mut self.config.gpu.auto_detect, "Auto-detect CUDA GPUs")
                .changed()
            {
                self.unsaved_changes = true;
            }
        });

        ui.add_space(15.0);

        // Config file location
        if let Ok(path) = Config::config_path() {
            ui.horizontal(|ui| {
                ui.label("Config file:");
                if ui.link(path.display().to_string()).clicked() {
                    if let Err(e) = open::that(&path) {
                        warn!("Failed to open config file: {}", e);
                    }
                }
            });
        }
    }
}

impl eframe::App for PreferencesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Hotkey, "Hotkey");
                ui.selectable_value(&mut self.active_tab, Tab::Transcription, "Transcription");
                ui.selectable_value(&mut self.active_tab, Tab::Audio, "Audio");
                ui.selectable_value(&mut self.active_tab, Tab::Output, "Output");
                ui.selectable_value(&mut self.active_tab, Tab::Appearance, "Appearance");
                ui.selectable_value(&mut self.active_tab, Tab::Advanced, "Advanced");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Save button
                let save_text = if self.unsaved_changes {
                    "Save *"
                } else {
                    "Save"
                };
                if ui.button(save_text).clicked() {
                    self.save_config();
                }

                // Status message
                if let Some((msg, time)) = &self.status_message {
                    if time.elapsed().as_secs() < 3 {
                        ui.label(msg);
                    } else {
                        self.status_message = None;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.unsaved_changes {
                        ui.label("Unsaved changes");
                    }
                });
            });
        });

        let ctx_clone = ctx.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.active_tab {
                Tab::Hotkey => self.show_hotkey_tab(ui),
                Tab::Transcription => self.show_transcription_tab(ui),
                Tab::Audio => self.show_audio_tab(ui),
                Tab::Output => self.show_output_tab(ui),
                Tab::Appearance => self.show_appearance_tab(ui, &ctx_clone),
                Tab::Advanced => self.show_advanced_tab(ui),
            });
        });
    }
}
