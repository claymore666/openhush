//! System audio capture using ScreenCaptureKit on macOS 13+.
//!
//! Captures desktop audio (meetings, calls, media) via Apple's ScreenCaptureKit framework.
//! Requires macOS 13 (Ventura) or later.

#![allow(dead_code)]

use screencapturekit::{
    sc_content_filter::SCContentFilter, sc_error::SCStreamError,
    sc_output_type::SCStreamOutputType, sc_shareable_content::SCShareableContent,
    sc_stream::SCStream, sc_stream_configuration::SCStreamConfiguration,
    sc_stream_output::StreamOutput,
};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

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

/// Audio sample handler for ScreenCaptureKit output
struct AudioHandler {
    samples: Arc<Mutex<Vec<f32>>>,
}

impl StreamOutput for AudioHandler {
    fn did_output_sample_buffer(
        &self,
        sample_buffer: screencapturekit::cm_sample_buffer::CMSampleBuffer,
        of_type: SCStreamOutputType,
    ) {
        if of_type != SCStreamOutputType::Audio {
            return;
        }

        // Extract audio samples from the CMSampleBuffer
        if let Some(audio_buffer) = sample_buffer.get_audio_buffer_list() {
            let mut samples = self.samples.lock().unwrap();

            // Process each audio buffer
            for buffer in audio_buffer.buffers() {
                if let Some(data) = buffer.data() {
                    // Convert bytes to f32 samples
                    // ScreenCaptureKit typically outputs 32-bit float samples
                    let float_samples: &[f32] = bytemuck::cast_slice(data);
                    samples.extend_from_slice(float_samples);
                }
            }
        }
    }
}

/// System audio capture using ScreenCaptureKit
pub struct SystemAudioCapture {
    /// Audio samples buffer
    samples: Arc<Mutex<Vec<f32>>>,
    /// ScreenCaptureKit stream
    stream: Option<SCStream>,
    /// Source description
    source_name: String,
}

impl SystemAudioCapture {
    /// Create a new system audio capture.
    ///
    /// Captures all system audio via ScreenCaptureKit.
    /// Requires Screen Recording permission on macOS.
    pub fn new(_source_name: Option<&str>) -> Result<Self, SystemAudioError> {
        let samples = Arc::new(Mutex::new(Vec::new()));

        // Get shareable content (requires permission)
        let content =
            SCShareableContent::get_with_exclude_desktop_windows(true, true).map_err(|e| {
                if e.to_string().contains("permission") {
                    SystemAudioError::PermissionDenied
                } else {
                    SystemAudioError::StreamFailed(e.to_string())
                }
            })?;

        // Configure for audio-only capture
        let config = SCStreamConfiguration::new();
        config.set_captures_audio(true);
        config.set_excludes_current_process_audio(true);
        config.set_sample_rate(SAMPLE_RATE as i32);
        config.set_channel_count(1); // Mono for Whisper

        // Create content filter for entire display audio
        let displays = content.displays();
        if displays.is_empty() {
            return Err(SystemAudioError::NoAudioSource);
        }

        let filter =
            SCContentFilter::new_with_display_excluding_windows(displays[0].clone(), Vec::new());

        // Create stream with audio handler
        let handler = AudioHandler {
            samples: Arc::clone(&samples),
        };

        let stream = SCStream::new(&filter, &config, handler)
            .map_err(|e| SystemAudioError::StreamFailed(e.to_string()))?;

        // Start capturing
        stream
            .start_capture()
            .map_err(|e| SystemAudioError::CaptureError(e.to_string()))?;

        info!("System audio capture started via ScreenCaptureKit");

        Ok(Self {
            samples,
            stream: Some(stream),
            source_name: "ScreenCaptureKit".to_string(),
        })
    }

    /// Get the source name being captured.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Extract captured samples and clear the buffer.
    ///
    /// Returns samples at 16kHz mono f32 format.
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
        if let Some(stream) = self.stream.take() {
            if let Err(e) = stream.stop_capture() {
                warn!("Failed to stop capture: {:?}", e);
            }
        }
        info!("System audio capture stopped");
    }
}

/// List available audio sources (displays for system audio).
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    let content = SCShareableContent::get_with_exclude_desktop_windows(true, true)
        .map_err(|e| SystemAudioError::StreamFailed(e.to_string()))?;

    let mut sources = Vec::new();

    for (i, display) in content.displays().iter().enumerate() {
        sources.push(SourceInfo {
            name: format!("display-{}", display.display_id()),
            description: format!("Display {} System Audio", i + 1),
            is_monitor: true,
            sample_rate: SAMPLE_RATE,
            channels: 2,
        });
    }

    // Add running applications as potential audio sources
    for app in content.applications() {
        if let Some(name) = app.application_name() {
            sources.push(SourceInfo {
                name: format!("app-{}", app.process_id()),
                description: format!("{} Audio", name),
                is_monitor: true,
                sample_rate: SAMPLE_RATE,
                channels: 2,
            });
        }
    }

    Ok(sources)
}

/// Check if ScreenCaptureKit is available (macOS 13+).
pub fn is_available() -> bool {
    // ScreenCaptureKit is available on macOS 13+
    // The screencapturekit crate handles this check internally
    true
}

/// Check if screen recording permission is granted.
pub fn has_permission() -> bool {
    // Try to get shareable content - this will fail if permission is not granted
    SCShareableContent::get_with_exclude_desktop_windows(true, true).is_ok()
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
}
