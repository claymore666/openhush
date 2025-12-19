//! GUI preferences window using egui.

use crate::config::Config;
use eframe::egui;
use tracing::info;

/// Run the preferences GUI as a standalone window
pub fn run_preferences() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 600.0])
            .with_min_inner_size([400.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "OpenHush Preferences",
        options,
        Box::new(|_cc| Ok(Box::new(PreferencesApp::new()))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run preferences: {}", e))
}

/// Spawn preferences window in a separate thread (for use from daemon)
pub fn spawn_preferences() {
    std::thread::spawn(|| {
        if let Err(e) = run_preferences() {
            tracing::error!("Preferences window error: {}", e);
        }
    });
}

struct PreferencesApp {
    config: Config,
    active_tab: Tab,
    unsaved_changes: bool,
    status_message: Option<(String, std::time::Instant)>,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Hotkey,
    Transcription,
    Audio,
    Output,
    Advanced,
}

impl PreferencesApp {
    fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        Self {
            config,
            active_tab: Tab::Hotkey,
            unsaved_changes: false,
            status_message: None,
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
        ui.heading("Transcription Settings");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Model:");
            let models = ["tiny", "base", "small", "medium", "large-v3"];
            egui::ComboBox::from_id_salt("model")
                .selected_text(&self.config.transcription.model)
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
                    let _ = open::that(&path);
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

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.active_tab {
                Tab::Hotkey => self.show_hotkey_tab(ui),
                Tab::Transcription => self.show_transcription_tab(ui),
                Tab::Audio => self.show_audio_tab(ui),
                Tab::Output => self.show_output_tab(ui),
                Tab::Advanced => self.show_advanced_tab(ui),
            });
        });
    }
}
