//! Audio capture using cpal.
//!
//! Captures audio from the microphone at 16kHz mono for Whisper compatibility.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleRate, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// Minimum recording duration in seconds
pub const MIN_DURATION_SECS: f32 = 0.5;

#[derive(Error, Debug)]
pub enum AudioRecorderError {
    #[error("No audio input device found")]
    NoInputDevice,

    #[error("Failed to get default input config: {0}")]
    NoInputConfig(String),

    #[error("Failed to build audio stream: {0}")]
    StreamBuildFailed(String),

    #[error("Failed to start audio stream: {0}")]
    StreamStartFailed(String),

    #[error("Recording too short (minimum {MIN_DURATION_SECS}s)")]
    TooShort,

    #[error("Not currently recording")]
    NotRecording,

    #[error("Already recording")]
    AlreadyRecording,
}

/// Audio buffer containing recorded samples
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Audio samples (f32, mono, 16kHz)
    pub samples: Vec<f32>,
    /// Sample rate
    pub sample_rate: u32,
}

impl AudioBuffer {
    /// Get duration in seconds
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }

    /// Check if buffer meets minimum duration
    pub fn is_valid(&self) -> bool {
        self.duration_secs() >= MIN_DURATION_SECS
    }

    /// Convert to i16 samples for Whisper
    #[allow(dead_code)]
    pub fn to_i16(&self) -> Vec<i16> {
        self.samples
            .iter()
            .map(|&s| (s * i16::MAX as f32) as i16)
            .collect()
    }
}

/// Audio recorder for capturing microphone input
pub struct AudioRecorder {
    device: Device,
    config: StreamConfig,
    recording: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<Stream>,
    device_sample_rate: u32,
}

impl AudioRecorder {
    /// Create a new audio recorder using the default input device
    pub fn new() -> Result<Self, AudioRecorderError> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or(AudioRecorderError::NoInputDevice)?;

        let device_name = device.name().unwrap_or_else(|_| "unknown".to_string());
        info!("Using audio input device: {}", device_name);

        // Get supported config
        let supported_config = device
            .default_input_config()
            .map_err(|e| AudioRecorderError::NoInputConfig(e.to_string()))?;

        let device_sample_rate = supported_config.sample_rate().0;
        info!(
            "Device sample rate: {} Hz (will resample to {} Hz)",
            device_sample_rate, SAMPLE_RATE
        );

        // Build config for mono capture
        let config = StreamConfig {
            channels: 1,
            sample_rate: SampleRate(device_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        Ok(Self {
            device,
            config,
            recording: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            device_sample_rate,
        })
    }

    /// List available audio input devices
    #[allow(dead_code)]
    pub fn list_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devices| {
                devices
                    .filter_map(|d| d.name().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Start recording audio
    pub fn start(&mut self) -> Result<(), AudioRecorderError> {
        if self.recording.load(Ordering::SeqCst) {
            return Err(AudioRecorderError::AlreadyRecording);
        }

        // Clear previous samples
        {
            let mut samples = self.samples.lock().unwrap();
            samples.clear();
        }

        self.recording.store(true, Ordering::SeqCst);

        let recording = self.recording.clone();
        let samples = self.samples.clone();

        // Build the input stream
        let err_fn = |err| error!("Audio stream error: {}", err);

        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if recording.load(Ordering::SeqCst) {
                        let mut samples_guard = samples.lock().unwrap();
                        samples_guard.extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioRecorderError::StreamBuildFailed(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioRecorderError::StreamStartFailed(e.to_string()))?;

        self.stream = Some(stream);
        debug!("Audio recording started");

        Ok(())
    }

    /// Stop recording and return the audio buffer
    pub fn stop(&mut self) -> Result<AudioBuffer, AudioRecorderError> {
        if !self.recording.load(Ordering::SeqCst) {
            return Err(AudioRecorderError::NotRecording);
        }

        self.recording.store(false, Ordering::SeqCst);

        // Drop the stream to stop recording
        self.stream = None;

        // Get the recorded samples
        let samples = {
            let samples_guard = self.samples.lock().unwrap();
            samples_guard.clone()
        };

        debug!(
            "Audio recording stopped: {} samples ({:.2}s at {} Hz)",
            samples.len(),
            samples.len() as f32 / self.device_sample_rate as f32,
            self.device_sample_rate
        );

        // Resample to 16kHz if needed
        let resampled = if self.device_sample_rate != SAMPLE_RATE {
            resample(&samples, self.device_sample_rate, SAMPLE_RATE)
        } else {
            samples
        };

        let buffer = AudioBuffer {
            samples: resampled,
            sample_rate: SAMPLE_RATE,
        };

        if !buffer.is_valid() {
            warn!(
                "Recording too short: {:.2}s (minimum {:.2}s)",
                buffer.duration_secs(),
                MIN_DURATION_SECS
            );
            return Err(AudioRecorderError::TooShort);
        }

        info!(
            "Captured audio: {:.2}s ({} samples)",
            buffer.duration_secs(),
            buffer.samples.len()
        );

        Ok(buffer)
    }

    /// Check if currently recording
    #[allow(dead_code)]
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }
}

/// Simple linear resampling
///
/// For better quality, consider using the `rubato` crate.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut result = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let src_idx_floor = src_idx.floor() as usize;
        let src_idx_ceil = (src_idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - src_idx_floor as f64;

        // Linear interpolation
        let sample = samples[src_idx_floor] * (1.0 - frac as f32)
            + samples[src_idx_ceil] * frac as f32;
        result.push(sample);
    }

    result
}

/// Save audio buffer to a WAV file for debugging
#[cfg(feature = "debug-audio")]
#[allow(dead_code)]
pub fn save_wav(buffer: &AudioBuffer, path: &std::path::Path) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(path)?;

    // WAV header
    let data_size = (buffer.samples.len() * 2) as u32; // 16-bit samples
    let file_size = 36 + data_size;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt chunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // Chunk size
    file.write_all(&1u16.to_le_bytes())?; // Audio format (PCM)
    file.write_all(&1u16.to_le_bytes())?; // Channels (mono)
    file.write_all(&buffer.sample_rate.to_le_bytes())?;
    file.write_all(&(buffer.sample_rate * 2).to_le_bytes())?; // Byte rate
    file.write_all(&2u16.to_le_bytes())?; // Block align
    file.write_all(&16u16.to_le_bytes())?; // Bits per sample

    // data chunk
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    // Write samples as 16-bit
    for sample in buffer.to_i16() {
        file.write_all(&sample.to_le_bytes())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_duration() {
        let buffer = AudioBuffer {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
        };
        assert!((buffer.duration_secs() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_buffer_validity() {
        let short_buffer = AudioBuffer {
            samples: vec![0.0; 4000], // 0.25s at 16kHz
            sample_rate: 16000,
        };
        assert!(!short_buffer.is_valid());

        let valid_buffer = AudioBuffer {
            samples: vec![0.0; 16000], // 1s at 16kHz
            sample_rate: 16000,
        };
        assert!(valid_buffer.is_valid());
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let result = resample(&samples, 16000, 16000);
        assert_eq!(result, samples);
    }

    #[test]
    fn test_resample_downsample() {
        let samples: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let result = resample(&samples, 48000, 16000);
        assert!(result.len() < samples.len());
    }
}
