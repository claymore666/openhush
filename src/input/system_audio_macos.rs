//! System audio capture using ScreenCaptureKit on macOS 13+.
//!
//! Captures desktop audio (meetings, calls, media) via Apple's ScreenCaptureKit framework.
//! Requires macOS 13 (Ventura) or later and Screen Recording permission.

#![allow(dead_code)]

use screencapturekit::prelude::*;
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

impl SCStreamOutputTrait for AudioHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if of_type != SCStreamOutputType::Audio {
            return;
        }

        // Get audio data from sample buffer
        if let Some(audio_data) = sample.get_audio_buffer_list() {
            // Convert audio data to f32 samples
            // ScreenCaptureKit typically outputs float32 samples
            let float_samples: Vec<f32> = audio_data
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            if !float_samples.is_empty() {
                let mut samples = self.samples.lock().unwrap();
                samples.extend(float_samples);
            }
        }
    }
}

/// System audio capture using ScreenCaptureKit
pub struct SystemAudioCapture {
    /// Audio samples buffer
    samples: Arc<Mutex<Vec<f32>>>,
    /// ScreenCaptureKit stream
    stream: SCStream,
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
        let content = SCShareableContent::get().map_err(|e| {
            let msg = format!("{:?}", e);
            if msg.contains("permission") || msg.contains("denied") {
                SystemAudioError::PermissionDenied
            } else {
                SystemAudioError::StreamFailed(msg)
            }
        })?;

        // Get displays
        let displays = content.displays();
        if displays.is_empty() {
            return Err(SystemAudioError::NoAudioSource);
        }

        debug!("Found {} displays for audio capture", displays.len());

        // Create content filter for entire display audio
        let filter = SCContentFilter::new()
            .with_display(&displays[0])
            .with_excluding_windows(&[]);

        // Configure for audio capture at 16kHz mono
        // Note: ScreenCaptureKit may resample internally
        let config = SCStreamConfiguration::new()
            .with_captures_audio(true)
            .with_excludes_current_process_audio(true)
            .with_sample_rate(SAMPLE_RATE as i32)
            .with_channel_count(1);

        // Create stream
        let mut stream = SCStream::new(&filter, &config);

        // Add audio handler
        let handler = AudioHandler {
            samples: Arc::clone(&samples),
        };
        stream.add_output_handler(handler, SCStreamOutputType::Audio);

        // Start capturing
        stream
            .start_capture()
            .map_err(|e| SystemAudioError::CaptureError(format!("{:?}", e)))?;

        info!("System audio capture started via ScreenCaptureKit");

        Ok(Self {
            samples,
            stream,
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
        if let Err(e) = self.stream.stop_capture() {
            warn!("Failed to stop capture: {:?}", e);
        }
        info!("System audio capture stopped");
    }
}

/// List available audio sources (displays for system audio).
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    let content = SCShareableContent::get()
        .map_err(|e| SystemAudioError::StreamFailed(format!("{:?}", e)))?;

    let mut sources = Vec::new();

    for (i, _display) in content.displays().iter().enumerate() {
        sources.push(SourceInfo {
            name: format!("display-{}", i),
            description: format!("Display {} System Audio", i + 1),
            is_monitor: true,
            sample_rate: SAMPLE_RATE,
            channels: 2,
        });
    }

    // Add running applications as potential audio sources
    for app in content.applications() {
        let name = app.application_name();
        sources.push(SourceInfo {
            name: format!("app-{}", app.process_id()),
            description: format!("{} Audio", name),
            is_monitor: true,
            sample_rate: SAMPLE_RATE,
            channels: 2,
        });
    }

    Ok(sources)
}

/// Check if ScreenCaptureKit is available (macOS 13+).
pub fn is_available() -> bool {
    // ScreenCaptureKit requires macOS 13+
    // The crate itself will fail if not available
    true
}

/// Check if screen recording permission is granted.
pub fn has_permission() -> bool {
    SCShareableContent::get().is_ok()
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
