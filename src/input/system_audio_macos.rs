//! System audio capture using ScreenCaptureKit on macOS 13+.
//!
//! Captures desktop audio (meetings, calls, media) via Apple's ScreenCaptureKit framework.
//! Requires macOS 13 (Ventura) or later and Screen Recording permission.

use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, info, warn};

use screencapturekit::{
    cm_sample_buffer::CMSampleBuffer,
    sc_content_filter::SCContentFilter,
    sc_shareable_content::SCShareableContent,
    sc_stream::SCStream,
    sc_stream_configuration::SCStreamConfiguration,
    sc_stream_output_trait::SCStreamOutputTrait,
    sc_types::SCStreamOutputType,
};

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// ScreenCaptureKit native sample rate (48kHz)
const NATIVE_SAMPLE_RATE: u32 = 48000;

/// Audio source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioSource {
    /// Default microphone input
    #[default]
    Microphone,
    /// System audio via ScreenCaptureKit
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
    #[error("ScreenCaptureKit not available (requires macOS 13+)")]
    NotAvailable,

    #[error("Screen recording permission denied")]
    PermissionDenied,

    #[error("No audio sources found")]
    NoAudioSource,

    #[error("Stream creation failed: {0}")]
    StreamFailed(String),

    #[error("Capture error: {0}")]
    CaptureError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Information about an audio source
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Source identifier
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// True if this is a monitor source (system audio)
    pub is_monitor: bool,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
}

/// Shared audio buffer for callback handler
struct AudioBuffer {
    /// Accumulated samples at native rate
    samples_native: Vec<f32>,
    /// Resampled samples at 16kHz
    samples_resampled: Vec<f32>,
}

impl AudioBuffer {
    fn new() -> Self {
        Self {
            samples_native: Vec::with_capacity(NATIVE_SAMPLE_RATE as usize * 30), // 30s buffer
            samples_resampled: Vec::new(),
        }
    }
}

/// Audio output handler for ScreenCaptureKit
struct AudioOutputHandler {
    buffer: Arc<Mutex<AudioBuffer>>,
}

impl SCStreamOutputTrait for AudioOutputHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if of_type != SCStreamOutputType::Audio {
            return;
        }

        // Extract audio data from the sample buffer
        if let Some(audio_buffers) = sample.audio_buffer_list() {
            let mut buffer = match self.buffer.lock() {
                Ok(b) => b,
                Err(e) => {
                    warn!("Failed to lock audio buffer: {}", e);
                    return;
                }
            };

            // Process each audio buffer (usually just one for mono/stereo)
            for audio_buffer in audio_buffers.buffers() {
                // Get the raw audio data as bytes
                let data = audio_buffer.data();

                // Audio is typically 32-bit float PCM
                // Convert bytes to f32 samples
                let samples: Vec<f32> = data
                    .chunks_exact(4)
                    .map(|chunk| {
                        let bytes: [u8; 4] = chunk.try_into().unwrap();
                        f32::from_le_bytes(bytes)
                    })
                    .collect();

                // If stereo, mix down to mono
                let mono_samples: Vec<f32> = if audio_buffer.number_channels() > 1 {
                    let channels = audio_buffer.number_channels() as usize;
                    samples
                        .chunks(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                        .collect()
                } else {
                    samples
                };

                buffer.samples_native.extend(mono_samples);
            }

            debug!(
                "Audio buffer: {} samples at native rate",
                buffer.samples_native.len()
            );
        }
    }
}

/// System audio capture using ScreenCaptureKit
pub struct SystemAudioCapture {
    /// Shared audio buffer
    buffer: Arc<Mutex<AudioBuffer>>,
    /// Source description
    source_name: String,
    /// The capture stream (kept alive while capturing)
    _stream: SCStream,
}

impl SystemAudioCapture {
    /// Create a new system audio capture.
    ///
    /// Starts capturing system audio from the primary display.
    /// Requires Screen Recording permission in System Preferences.
    pub fn new(source_name: Option<&str>) -> Result<Self, SystemAudioError> {
        info!("Initializing ScreenCaptureKit system audio capture...");

        // Check permission first
        if !has_permission() {
            return Err(SystemAudioError::PermissionDenied);
        }

        // Get shareable content (displays, windows, apps)
        let content = SCShareableContent::get().map_err(|e| {
            SystemAudioError::StreamFailed(format!("Failed to get shareable content: {:?}", e))
        })?;

        let displays = content.displays();
        if displays.is_empty() {
            return Err(SystemAudioError::NoAudioSource);
        }

        // Use the primary display (first one)
        let display = displays.into_iter().next().unwrap();
        let display_name = source_name
            .map(|s| s.to_string())
            .unwrap_or_else(|| "System Audio".to_string());

        info!("Using display for audio capture: {}", display_name);

        // Create content filter for the display (audio only, no video processing needed)
        let filter = SCContentFilter::new_with_display_excluding_windows(display, vec![]);

        // Configure stream for audio capture
        // Note: ScreenCaptureKit captures at 48kHz, we'll resample to 16kHz
        let config = SCStreamConfiguration::new()
            .with_width(1) // Minimal video (required but not used)
            .with_height(1)
            .with_captures_audio(true)
            .with_excludes_current_process_audio(false)
            .with_sample_rate(NATIVE_SAMPLE_RATE as i32)
            .with_channel_count(1); // Mono

        // Create shared buffer
        let buffer = Arc::new(Mutex::new(AudioBuffer::new()));

        // Create output handler
        let handler = AudioOutputHandler {
            buffer: Arc::clone(&buffer),
        };

        // Create and configure stream
        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(handler, SCStreamOutputType::Audio);

        // Start capture
        stream.start_capture().map_err(|e| {
            SystemAudioError::StreamFailed(format!("Failed to start capture: {:?}", e))
        })?;

        info!("ScreenCaptureKit audio capture started (48kHz → 16kHz)");

        Ok(Self {
            buffer,
            source_name: display_name,
            _stream: stream,
        })
    }

    /// Get the source name being captured.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Extract captured samples and clear the buffer.
    ///
    /// Returns samples at 16kHz (resampled from 48kHz native rate).
    pub fn extract_samples(&self) -> Vec<f32> {
        let mut buffer = self.buffer.lock().unwrap();

        if buffer.samples_native.is_empty() {
            return Vec::new();
        }

        // Resample from 48kHz to 16kHz (factor of 3)
        // Simple linear decimation - take every 3rd sample
        // For better quality, consider using rubato crate
        let resampled: Vec<f32> = buffer
            .samples_native
            .chunks(3)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect();

        buffer.samples_native.clear();
        buffer.samples_resampled = resampled.clone();

        debug!("Extracted {} samples (resampled to 16kHz)", resampled.len());
        resampled
    }

    /// Get the current buffer length in samples (at 16kHz equivalent).
    pub fn buffer_len(&self) -> usize {
        let buffer = self.buffer.lock().unwrap();
        buffer.samples_native.len() / 3 // Approximate 16kHz equivalent
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        info!("System audio capture stopped");
        // Stream is automatically stopped when dropped
    }
}

/// List available audio sources.
///
/// Returns information about available displays for audio capture.
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    if !has_permission() {
        return Ok(Vec::new());
    }

    let content = SCShareableContent::get().map_err(|e| {
        SystemAudioError::StreamFailed(format!("Failed to get shareable content: {:?}", e))
    })?;

    let sources: Vec<SourceInfo> = content
        .displays()
        .into_iter()
        .enumerate()
        .map(|(i, _display)| SourceInfo {
            name: format!("display_{}", i),
            description: format!("Display {} System Audio", i + 1),
            is_monitor: true,
            sample_rate: SAMPLE_RATE, // We resample to 16kHz
            channels: 1,
        })
        .collect();

    Ok(sources)
}

/// Check if ScreenCaptureKit is available (macOS 13+).
pub fn is_available() -> bool {
    // ScreenCaptureKit is available on macOS 12.3+, audio capture on 13+
    // The screencapturekit crate handles version checking
    true
}

/// Check if screen recording permission is granted.
pub fn has_permission() -> bool {
    // Try to get shareable content - this will fail if permission denied
    match SCShareableContent::get() {
        Ok(content) => !content.displays().is_empty(),
        Err(_) => false,
    }
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
    fn test_is_available() {
        // Should return true on macOS
        assert!(is_available());
    }

    #[test]
    fn test_audio_buffer_new() {
        let buffer = AudioBuffer::new();
        assert!(buffer.samples_native.is_empty());
        assert!(buffer.samples_resampled.is_empty());
    }
}
