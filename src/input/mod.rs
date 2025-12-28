//! Input handling: hotkey detection and audio capture.

pub mod audio;
pub mod hotkey;
pub mod ring_buffer;
#[cfg(target_os = "linux")]
pub mod system_audio;
pub mod wake_word;

#[allow(unused_imports)]
pub use audio::{load_wav_file, AudioBuffer, AudioRecorder, AudioRecorderError, ChannelMix};
pub use hotkey::{HotkeyEvent, HotkeyListener, HotkeyListenerError};
pub use ring_buffer::AudioMark;
#[allow(unused_imports)]
pub use ring_buffer::AudioRingBuffer;
#[allow(unused_imports)]
#[cfg(target_os = "linux")]
pub use system_audio::{AudioSource, SourceInfo, SystemAudioCapture, SystemAudioError};

use serde::{Deserialize, Serialize};

/// Type of audio input device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioDeviceType {
    /// Physical microphone input
    Microphone,
    /// System audio monitor (loopback)
    Monitor,
}

/// Information about an audio input device and its channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// Unique device identifier (platform-specific)
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Device type (microphone or monitor)
    pub device_type: AudioDeviceType,
    /// Number of available channels
    pub channel_count: u8,
    /// Channel names (e.g., "Left", "Right", "Channel 1")
    pub channel_names: Vec<String>,
    /// Sample rate
    pub sample_rate: u32,
    /// Whether this is the default device
    pub is_default: bool,
}

impl AudioDeviceInfo {
    /// Generate default channel names based on count
    pub fn default_channel_names(count: u8) -> Vec<String> {
        match count {
            1 => vec!["Mono".to_string()],
            2 => vec!["Left".to_string(), "Right".to_string()],
            _ => (1..=count).map(|i| format!("Channel {}", i)).collect(),
        }
    }
}

/// Channel selection for a specific device
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceChannelSelection {
    /// Device ID
    pub device_id: String,
    /// Selected channel indices (0-based)
    pub selected_channels: Vec<u8>,
    /// Whether this device is enabled for capture
    pub enabled: bool,
}

/// Enumerate all available audio input devices with channel information
#[cfg(target_os = "linux")]
pub fn enumerate_audio_inputs() -> Vec<AudioDeviceInfo> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let mut devices = Vec::new();

    // Get microphones via cpal
    let host = cpal::default_host();
    let default_id = host.default_input_device().and_then(|d| d.id().ok());

    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            let id = match device.id() {
                Ok(id) => id.to_string(),
                Err(_) => continue,
            };
            let name = device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| id.clone());
            let config = match device.default_input_config() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let channel_count = config.channels() as u8;
            let is_default = default_id.as_ref().map(|d| d.to_string()) == Some(id.clone());

            devices.push(AudioDeviceInfo {
                id: id.clone(),
                name,
                device_type: AudioDeviceType::Microphone,
                channel_count,
                channel_names: AudioDeviceInfo::default_channel_names(channel_count),
                sample_rate: config.sample_rate(),
                is_default,
            });
        }
    }

    // Get monitor sources via PulseAudio
    if let Ok(sources) = system_audio::list_monitor_sources() {
        for source in sources {
            devices.push(AudioDeviceInfo {
                id: source.name.clone(),
                name: source.description,
                device_type: AudioDeviceType::Monitor,
                channel_count: source.channels,
                channel_names: AudioDeviceInfo::default_channel_names(source.channels),
                sample_rate: source.sample_rate,
                is_default: false,
            });
        }
    }

    devices
}

/// Enumerate all available audio input devices (non-Linux stub)
#[cfg(not(target_os = "linux"))]
pub fn enumerate_audio_inputs() -> Vec<AudioDeviceInfo> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let mut devices = Vec::new();
    let host = cpal::default_host();
    let default_id = host.default_input_device().and_then(|d| d.id().ok());

    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            let id = match device.id() {
                Ok(id) => id.to_string(),
                Err(_) => continue,
            };
            let name = device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| id.clone());
            let config = match device.default_input_config() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let channel_count = config.channels() as u8;
            let is_default = default_id.as_ref().map(|d| d.to_string()) == Some(id.clone());

            devices.push(AudioDeviceInfo {
                id: id.clone(),
                name,
                device_type: AudioDeviceType::Microphone,
                channel_count,
                channel_names: AudioDeviceInfo::default_channel_names(channel_count),
                sample_rate: config.sample_rate(),
                is_default,
            });
        }
    }

    devices
}
