//! System audio capture using WASAPI loopback on Windows.
//!
//! Captures desktop audio (meetings, calls, media) via Windows Audio Session API.
//! Works on Windows Vista and later.
//!
//! ## Implementation Status
//!
//! This is currently a **stub implementation** that returns `NotImplemented` error.
//! The actual WASAPI integration needs to be implemented and tested on Windows hardware.
//!
//! ## Required Implementation Steps
//!
//! 1. Use `wasapi` crate (version 0.22+)
//! 2. Initialize COM: `initialize_mta()` returns HRESULT (check with `.is_ok()`)
//! 3. Get device enumerator: `DeviceEnumerator::new()`
//! 4. Get render device: `enumerator.get_default_device(&Direction::Render)`
//! 5. Create AudioClient: `device.get_iaudioclient()`
//! 6. Get mix format: `audio_client.get_mixformat()`
//! 7. Initialize client with StreamMode (not ShareMode):
//!    `audio_client.initialize_client(&format, &Direction::Capture, &StreamMode::EventsShared { .. })`
//! 8. Get capture client: `audio_client.get_audiocaptureclient()`
//! 9. Start stream and read samples
//!
//! ## Key API Differences from Assumptions
//!
//! - `initialize_mta()` returns `HRESULT`, not `Result<(), Error>`
//! - `initialize_client()` takes 3 args: `(&WaveFormat, &Direction, &StreamMode)`
//! - `StreamMode` enum variants: `EventsShared`, `EventsExclusive`, `PollingShared`, `PollingExclusive`
//! - For loopback, may need `new_application_loopback_client()` or AUDCLNT_STREAMFLAGS_LOOPBACK
//! - `DeviceCollection` needs `&` to iterate: `for device in &devices`
//! - Use `get_subformat()` not `get_subformat_guid()`

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
    ///
    /// ## Implementation Guide
    ///
    /// ```ignore
    /// use wasapi::*;
    ///
    /// // 1. Initialize COM (returns HRESULT, not Result)
    /// let hr = initialize_mta();
    /// if !hr.is_ok() {
    ///     return Err(SystemAudioError::InitFailed("COM init failed".into()));
    /// }
    ///
    /// // 2. Get device enumerator
    /// let enumerator = DeviceEnumerator::new()?;
    ///
    /// // 3. Get default render device
    /// let device = enumerator.get_default_device(&Direction::Render)?;
    ///
    /// // 4. Create audio client
    /// let mut audio_client = device.get_iaudioclient()?;
    ///
    /// // 5. Get mix format
    /// let format = audio_client.get_mixformat()?;
    ///
    /// // 6. Initialize with StreamMode (3 args, not 5)
    /// // For loopback, may need AUDCLNT_STREAMFLAGS_LOOPBACK flag
    /// let stream_mode = StreamMode::EventsShared {
    ///     buffer_duration: 100_000, // 10ms in 100ns units
    ///     autoconvert: true,
    /// };
    /// audio_client.initialize_client(&format, &Direction::Capture, &stream_mode)?;
    ///
    /// // 7. Get capture client
    /// let capture_client = audio_client.get_audiocaptureclient()?;
    ///
    /// // 8. Start and capture
    /// audio_client.start_stream()?;
    /// // In loop: capture_client.read_from_device_to_deque(&mut buffer)
    /// ```
    pub fn new(_device_name: Option<&str>) -> Result<Self, SystemAudioError> {
        Err(SystemAudioError::NotImplemented(
            "Windows system audio capture requires WASAPI implementation. \
             Use microphone input instead, or implement WASAPI integration. \
             See module documentation for implementation guide."
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
    Ok(Vec::new())
}

/// Check if WASAPI loopback is available.
pub fn is_available() -> bool {
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
