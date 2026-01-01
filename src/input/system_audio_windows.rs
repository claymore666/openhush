//! System audio capture using WASAPI loopback on Windows.
//!
//! Captures desktop audio (meetings, calls, media) via Windows Audio Session API.
//! Works on Windows Vista and later.
//!
//! NOTE: This is a stub implementation. The actual WASAPI integration
//! needs to be implemented and tested on Windows hardware.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::info;

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// Audio source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioSource {
    /// Default microphone input
    #[default]
    Microphone,
    /// System audio via WASAPI loopback
    Monitor,
    /// Both microphone and system audio mixed
    Both,
}

impl std::str::FromStr for AudioSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mic" | "microphone" => Ok(Self::Microphone),
            "monitor" | "system" | "desktop" => Ok(Self::Monitor),
            "both" | "mix" | "all" => Ok(Self::Both),
            _ => Err(format!(
                "Unknown audio source '{}'. Use: mic, monitor, or both",
                s
            )),
        }
    }
}

/// Errors from system audio capture
#[derive(Error, Debug)]
pub enum SystemAudioError {
    #[error("WASAPI initialization failed: {0}")]
    InitFailed(String),

    #[error("No audio output device found")]
    NoOutputDevice,

    #[error("Loopback capture not supported")]
    LoopbackNotSupported,

    #[error("Stream creation failed: {0}")]
    StreamFailed(String),

    #[error("Capture error: {0}")]
    CaptureError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Information about a WASAPI audio device
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Device ID
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// True if this is a loopback source (system audio)
    pub is_monitor: bool,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
}

/// System audio capture using WASAPI loopback
///
/// NOTE: This is a stub. Real implementation requires WASAPI integration.
pub struct SystemAudioCapture {
    /// Audio samples buffer
    samples: Arc<Mutex<Vec<f32>>>,
    /// Device being captured
    device_name: String,
}

impl SystemAudioCapture {
    /// Create a new system audio capture from the default output device.
    ///
    /// NOTE: Currently returns NotImplemented error.
    /// Real implementation requires WASAPI on Windows.
    pub fn new(_device_name: Option<&str>) -> Result<Self, SystemAudioError> {
        // TODO: Implement using wasapi crate
        // The actual implementation requires:
        // 1. wasapi::initialize_mta() to initialize COM
        // 2. DeviceEnumerator to get render devices
        // 3. AudioClient for loopback capture
        // 4. Proper handling of StreamMode (not ShareMode)
        //
        // See: https://docs.rs/wasapi/latest/wasapi/

        Err(SystemAudioError::NotImplemented(
            "Windows system audio capture requires WASAPI implementation. \
             Use microphone input instead, or implement WASAPI integration."
                .to_string(),
        ))
    }

    /// Get the device name being captured.
    pub fn source_name(&self) -> &str {
        &self.device_name
    }

    /// Extract captured samples and clear the buffer.
    pub fn extract_samples(&self) -> Vec<f32> {
        let mut samples = self.samples.lock().unwrap();
        std::mem::take(&mut *samples)
    }

    /// Get the current buffer length in samples.
    pub fn buffer_len(&self) -> usize {
        self.samples.lock().unwrap().len()
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        info!("System audio capture stopped");
    }
}

/// List available audio output devices (for loopback capture).
///
/// NOTE: Currently returns empty list. Real implementation needs WASAPI.
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    // TODO: Use DeviceEnumerator to enumerate render devices
    Ok(Vec::new())
}

/// Check if WASAPI loopback is available.
pub fn is_available() -> bool {
    // WASAPI is available on Windows Vista+
    // For now, return true and let actual usage fail with proper error
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_source_from_str() {
        assert_eq!(
            "mic".parse::<AudioSource>().unwrap(),
            AudioSource::Microphone
        );
        assert_eq!(
            "monitor".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );
        assert_eq!("both".parse::<AudioSource>().unwrap(), AudioSource::Both);
        assert!("invalid".parse::<AudioSource>().is_err());
    }

    #[test]
    fn test_new_returns_not_implemented() {
        let result = SystemAudioCapture::new(None);
        assert!(matches!(result, Err(SystemAudioError::NotImplemented(_))));
    }
}
