//! Audio channel selector GUI - mixer-style window for selecting input channels.

use crate::input::{
    enumerate_audio_inputs, AudioDeviceInfo, AudioDeviceType, DeviceChannelSelection,
};
use eframe::egui;
use std::collections::HashMap;

/// Run the channel selector GUI as a modal window
/// Returns the selected device channels when the user confirms, or None if cancelled
pub fn run_channel_selector(
    current_selections: &[DeviceChannelSelection],
) -> Option<Vec<DeviceChannelSelection>> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 500.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("Audio Input Channels"),
        ..Default::default()
    };

    let mut result: Option<Vec<DeviceChannelSelection>> = None;
    let result_ptr = &mut result as *mut Option<Vec<DeviceChannelSelection>>;

    let _ = eframe::run_native(
        "Audio Input Channels",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(ChannelSelectorApp::new(
                current_selections,
                result_ptr,
            )))
        }),
    );

    result
}

/// Spawn the channel selector as a separate process
#[allow(dead_code)]
pub fn spawn_channel_selector() {
    let exe = std::env::current_exe().unwrap_or_else(|_| "openhush".into());
    match std::process::Command::new(exe)
        .arg("audio-channels")
        .spawn()
    {
        Ok(_) => tracing::info!("Audio channel selector spawned"),
        Err(e) => tracing::error!("Failed to spawn channel selector: {}", e),
    }
}

struct ChannelSelectorApp {
    /// All available audio devices
    devices: Vec<AudioDeviceInfo>,
    /// Selection state per device
    selections: HashMap<String, DeviceSelection>,
    /// Pointer to write result on confirm
    result_ptr: *mut Option<Vec<DeviceChannelSelection>>,
    /// Search/filter text
    filter_text: String,
    /// Show only devices with selected channels
    show_only_selected: bool,
}

struct DeviceSelection {
    /// Whether this device is enabled
    enabled: bool,
    /// Per-channel selection state (true = selected)
    channels: Vec<bool>,
}

// SAFETY: result_ptr is only accessed from the main thread during app lifecycle
unsafe impl Send for ChannelSelectorApp {}

impl ChannelSelectorApp {
    fn new(
        current_selections: &[DeviceChannelSelection],
        result_ptr: *mut Option<Vec<DeviceChannelSelection>>,
    ) -> Self {
        let devices = enumerate_audio_inputs();
        let mut selections = HashMap::new();

        // Initialize selection state from current config
        for device in &devices {
            let existing = current_selections.iter().find(|s| s.device_id == device.id);

            let (enabled, channels) = if let Some(sel) = existing {
                let mut ch = vec![false; device.channel_count as usize];
                for &idx in &sel.selected_channels {
                    if (idx as usize) < ch.len() {
                        ch[idx as usize] = true;
                    }
                }
                (sel.enabled, ch)
            } else {
                // Default: device disabled, all channels deselected
                (false, vec![false; device.channel_count as usize])
            };

            selections.insert(device.id.clone(), DeviceSelection { enabled, channels });
        }

        Self {
            devices,
            selections,
            result_ptr,
            filter_text: String::new(),
            show_only_selected: false,
        }
    }

    fn build_result(&self) -> Vec<DeviceChannelSelection> {
        self.selections
            .iter()
            .filter(|(_, sel)| sel.enabled)
            .map(|(device_id, sel)| {
                let selected_channels: Vec<u8> = sel
                    .channels
                    .iter()
                    .enumerate()
                    .filter(|(_, &selected)| selected)
                    .map(|(i, _)| i as u8)
                    .collect();

                DeviceChannelSelection {
                    device_id: device_id.clone(),
                    selected_channels,
                    enabled: sel.enabled,
                }
            })
            .collect()
    }

    fn show_device_by_index(&mut self, ui: &mut egui::Ui, idx: usize) {
        // Clone device data to avoid borrow conflicts
        let device = match self.devices.get(idx) {
            Some(d) => d.clone(),
            None => return,
        };

        let selection = match self.selections.get_mut(&device.id) {
            Some(s) => s,
            None => return,
        };

        let device_icon = match device.device_type {
            AudioDeviceType::Microphone => "🎤",
            AudioDeviceType::Monitor => "🔊",
        };

        let header_text = format!(
            "{} {} ({} ch, {} Hz){}",
            device_icon,
            device.name,
            device.channel_count,
            device.sample_rate,
            if device.is_default { " [Default]" } else { "" }
        );

        egui::CollapsingHeader::new(&header_text)
            .default_open(selection.enabled)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut selection.enabled, "Enable this device");

                    if selection.enabled {
                        if ui.button("Select All").clicked() {
                            for ch in &mut selection.channels {
                                *ch = true;
                            }
                        }
                        if ui.button("Deselect All").clicked() {
                            for ch in &mut selection.channels {
                                *ch = false;
                            }
                        }
                    }
                });

                if selection.enabled {
                    ui.add_space(5.0);
                    ui.label("Channels:");

                    // Show channels in a grid
                    ui.horizontal_wrapped(|ui| {
                        for (i, channel_name) in device.channel_names.iter().enumerate() {
                            if let Some(ch) = selection.channels.get_mut(i) {
                                ui.checkbox(ch, channel_name);
                            }
                        }
                    });

                    let selected_count = selection.channels.iter().filter(|&&c| c).count();
                    ui.label(format!(
                        "Selected: {} / {} channels",
                        selected_count, device.channel_count
                    ));
                }
            });
    }
}

impl eframe::App for ChannelSelectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.heading("Audio Input Channels");
            ui.add_space(5.0);
            ui.label("Select which audio channels to mix for transcription.");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut self.filter_text);

                ui.separator();
                ui.checkbox(&mut self.show_only_selected, "Show only enabled");

                if ui.button("🔄 Refresh").clicked() {
                    self.devices = enumerate_audio_inputs();
                }
            });
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                if ui.button("Apply").clicked() {
                    // Write result and close
                    // SAFETY: result_ptr is valid for the lifetime of the app
                    unsafe {
                        *self.result_ptr = Some(self.build_result());
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let enabled_count = self.selections.values().filter(|s| s.enabled).count();
                    let total_channels: usize = self
                        .selections
                        .values()
                        .filter(|s| s.enabled)
                        .map(|s| s.channels.iter().filter(|&&c| c).count())
                        .sum();
                    ui.label(format!(
                        "{} device(s), {} channel(s) selected",
                        enabled_count, total_channels
                    ));
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.devices.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label("No audio input devices found.");
                    ui.add_space(10.0);
                    if ui.button("Refresh").clicked() {
                        self.devices = enumerate_audio_inputs();
                    }
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                let filter = self.filter_text.to_lowercase();

                // Collect device indices by type to avoid borrow conflicts
                let mic_indices: Vec<usize> = self
                    .devices
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.device_type == AudioDeviceType::Microphone)
                    .filter(|(_, d)| filter.is_empty() || d.name.to_lowercase().contains(&filter))
                    .filter(|(_, d)| {
                        if self.show_only_selected {
                            self.selections
                                .get(&d.id)
                                .map(|s| s.enabled)
                                .unwrap_or(false)
                        } else {
                            true
                        }
                    })
                    .map(|(i, _)| i)
                    .collect();

                let mon_indices: Vec<usize> = self
                    .devices
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.device_type == AudioDeviceType::Monitor)
                    .filter(|(_, d)| filter.is_empty() || d.name.to_lowercase().contains(&filter))
                    .filter(|(_, d)| {
                        if self.show_only_selected {
                            self.selections
                                .get(&d.id)
                                .map(|s| s.enabled)
                                .unwrap_or(false)
                        } else {
                            true
                        }
                    })
                    .map(|(i, _)| i)
                    .collect();

                // Microphones section
                if !mic_indices.is_empty() {
                    ui.heading("🎤 Microphones");
                    ui.add_space(5.0);

                    for idx in mic_indices {
                        self.show_device_by_index(ui, idx);
                    }
                    ui.add_space(10.0);
                }

                // Monitors section (system audio)
                if !mon_indices.is_empty() {
                    ui.heading("🔊 System Audio (Monitor)");
                    ui.add_space(5.0);

                    for idx in mon_indices {
                        self.show_device_by_index(ui, idx);
                    }
                }
            });
        });
    }
}
