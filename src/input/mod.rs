//! Input handling: hotkey detection and audio capture.

pub mod audio;
pub mod hotkey;
pub mod ring_buffer;
#[cfg(target_os = "linux")]
pub mod system_audio;
// ScreenCaptureKit only works on aarch64 (Apple Silicon) due to Swift cross-compilation issues
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub mod system_audio_macos;
#[cfg(any(target_os = "windows", all(target_os = "macos", target_arch = "x86_64")))]
pub mod system_audio_windows;
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
#[allow(unused_imports)]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub use system_audio_macos::{AudioSource, SourceInfo, SystemAudioCapture, SystemAudioError};
#[allow(unused_imports)]
#[cfg(any(target_os = "windows", all(target_os = "macos", target_arch = "x86_64")))]
pub use system_audio_windows::{AudioSource, SourceInfo, SystemAudioCapture, SystemAudioError};

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

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // AudioDeviceType Tests
    // ===================

    #[test]
    fn test_audio_device_type_debug() {
        let mic = AudioDeviceType::Microphone;
        let mon = AudioDeviceType::Monitor;
        assert_eq!(format!("{:?}", mic), "Microphone");
        assert_eq!(format!("{:?}", mon), "Monitor");
    }

    #[test]
    fn test_audio_device_type_clone() {
        let original = AudioDeviceType::Microphone;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_audio_device_type_eq() {
        assert_eq!(AudioDeviceType::Microphone, AudioDeviceType::Microphone);
        assert_eq!(AudioDeviceType::Monitor, AudioDeviceType::Monitor);
        assert_ne!(AudioDeviceType::Microphone, AudioDeviceType::Monitor);
    }

    #[test]
    fn test_audio_device_type_serialize() {
        let mic = AudioDeviceType::Microphone;
        let json = serde_json::to_string(&mic).unwrap();
        assert_eq!(json, "\"Microphone\"");

        let mon = AudioDeviceType::Monitor;
        let json = serde_json::to_string(&mon).unwrap();
        assert_eq!(json, "\"Monitor\"");
    }

    #[test]
    fn test_audio_device_type_deserialize() {
        let mic: AudioDeviceType = serde_json::from_str("\"Microphone\"").unwrap();
        assert_eq!(mic, AudioDeviceType::Microphone);

        let mon: AudioDeviceType = serde_json::from_str("\"Monitor\"").unwrap();
        assert_eq!(mon, AudioDeviceType::Monitor);
    }

    // ===================
    // AudioDeviceInfo Tests
    // ===================

    #[test]
    fn test_audio_device_info_default_channel_names_mono() {
        let names = AudioDeviceInfo::default_channel_names(1);
        assert_eq!(names, vec!["Mono"]);
    }

    #[test]
    fn test_audio_device_info_default_channel_names_stereo() {
        let names = AudioDeviceInfo::default_channel_names(2);
        assert_eq!(names, vec!["Left", "Right"]);
    }

    #[test]
    fn test_audio_device_info_default_channel_names_multi() {
        let names = AudioDeviceInfo::default_channel_names(4);
        assert_eq!(
            names,
            vec!["Channel 1", "Channel 2", "Channel 3", "Channel 4"]
        );
    }

    #[test]
    fn test_audio_device_info_default_channel_names_eight() {
        let names = AudioDeviceInfo::default_channel_names(8);
        assert_eq!(names.len(), 8);
        assert_eq!(names[0], "Channel 1");
        assert_eq!(names[7], "Channel 8");
    }

    #[test]
    fn test_audio_device_info_clone() {
        let info = AudioDeviceInfo {
            id: "test-device".to_string(),
            name: "Test Microphone".to_string(),
            device_type: AudioDeviceType::Microphone,
            channel_count: 2,
            channel_names: vec!["Left".to_string(), "Right".to_string()],
            sample_rate: 48000,
            is_default: true,
        };
        let cloned = info.clone();
        assert_eq!(info.id, cloned.id);
        assert_eq!(info.name, cloned.name);
        assert_eq!(info.device_type, cloned.device_type);
        assert_eq!(info.channel_count, cloned.channel_count);
        assert_eq!(info.channel_names, cloned.channel_names);
        assert_eq!(info.sample_rate, cloned.sample_rate);
        assert_eq!(info.is_default, cloned.is_default);
    }

    #[test]
    fn test_audio_device_info_serialize() {
        let info = AudioDeviceInfo {
            id: "dev1".to_string(),
            name: "Mic".to_string(),
            device_type: AudioDeviceType::Microphone,
            channel_count: 1,
            channel_names: vec!["Mono".to_string()],
            sample_rate: 16000,
            is_default: false,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("dev1"));
        assert!(json.contains("Mic"));
        assert!(json.contains("Microphone"));
    }

    #[test]
    fn test_audio_device_info_deserialize() {
        let json = r#"{
            "id": "test-id",
            "name": "Test Device",
            "device_type": "Monitor",
            "channel_count": 2,
            "channel_names": ["Left", "Right"],
            "sample_rate": 44100,
            "is_default": true
        }"#;
        let info: AudioDeviceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "test-id");
        assert_eq!(info.name, "Test Device");
        assert_eq!(info.device_type, AudioDeviceType::Monitor);
        assert_eq!(info.channel_count, 2);
        assert_eq!(info.channel_names, vec!["Left", "Right"]);
        assert_eq!(info.sample_rate, 44100);
        assert!(info.is_default);
    }

    // ===================
    // DeviceChannelSelection Tests
    // ===================

    #[test]
    fn test_device_channel_selection_default() {
        let sel = DeviceChannelSelection::default();
        assert!(sel.device_id.is_empty());
        assert!(sel.selected_channels.is_empty());
        assert!(!sel.enabled);
    }

    #[test]
    fn test_device_channel_selection_clone() {
        let sel = DeviceChannelSelection {
            device_id: "mic1".to_string(),
            selected_channels: vec![0, 1],
            enabled: true,
        };
        let cloned = sel.clone();
        assert_eq!(sel.device_id, cloned.device_id);
        assert_eq!(sel.selected_channels, cloned.selected_channels);
        assert_eq!(sel.enabled, cloned.enabled);
    }

    #[test]
    fn test_device_channel_selection_serialize() {
        let sel = DeviceChannelSelection {
            device_id: "device-abc".to_string(),
            selected_channels: vec![0, 2, 4],
            enabled: true,
        };
        let json = serde_json::to_string(&sel).unwrap();
        assert!(json.contains("device-abc"));
        assert!(json.contains("[0,2,4]"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_device_channel_selection_deserialize() {
        let json = r#"{
            "device_id": "test-dev",
            "selected_channels": [1, 3],
            "enabled": false
        }"#;
        let sel: DeviceChannelSelection = serde_json::from_str(json).unwrap();
        assert_eq!(sel.device_id, "test-dev");
        assert_eq!(sel.selected_channels, vec![1, 3]);
        assert!(!sel.enabled);
    }

    #[test]
    fn test_device_channel_selection_roundtrip() {
        let original = DeviceChannelSelection {
            device_id: "roundtrip-test".to_string(),
            selected_channels: vec![0, 1, 2, 3],
            enabled: true,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: DeviceChannelSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(original.device_id, parsed.device_id);
        assert_eq!(original.selected_channels, parsed.selected_channels);
        assert_eq!(original.enabled, parsed.enabled);
    }

    // ===================
    // enumerate_audio_inputs Tests
    // ===================

    #[test]
    fn test_enumerate_audio_inputs_no_panic() {
        // Just verify the function doesn't panic
        // Result depends on available hardware
        let _ = enumerate_audio_inputs();
    }

    #[test]
    fn test_enumerate_audio_inputs_returns_vec() {
        let devices = enumerate_audio_inputs();
        // Should return a Vec (may be empty if no devices)
        // All devices should have valid data
        for device in &devices {
            assert!(!device.id.is_empty());
            assert!(!device.name.is_empty());
            assert!(device.channel_count > 0);
            assert_eq!(device.channel_names.len(), device.channel_count as usize);
            assert!(device.sample_rate > 0);
        }
    }
}
