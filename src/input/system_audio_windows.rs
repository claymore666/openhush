//! System audio capture using WASAPI loopback on Windows.
//!
//! Captures desktop audio (meetings, calls, media) via Windows Audio Session API.
//! Works on Windows Vista and later.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use wasapi::{Direction, ShareMode, WaveFormat};

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
pub struct SystemAudioCapture {
    /// Audio samples buffer
    samples: Arc<Mutex<Vec<f32>>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Capture thread handle
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Device being captured
    device_name: String,
}

impl SystemAudioCapture {
    /// Create a new system audio capture from the default output device.
    ///
    /// Uses WASAPI loopback to capture what's playing through speakers.
    pub fn new(device_name: Option<&str>) -> Result<Self, SystemAudioError> {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = Arc::clone(&samples);

        // Initialize WASAPI
        wasapi::initialize_mta().map_err(|e| SystemAudioError::InitFailed(e.to_string()))?;

        // Get the default render device for loopback
        let device = wasapi::get_default_device(&Direction::Render)
            .map_err(|e| SystemAudioError::NoOutputDevice)?;

        let device_friendly_name = device
            .get_friendlyname()
            .unwrap_or_else(|_| "Unknown".to_string());

        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_capture_loop(device, samples_clone, shutdown_rx) {
                error!("WASAPI capture error: {}", e);
            }
        });

        info!(
            "System audio capture started from: {}",
            device_friendly_name
        );

        Ok(Self {
            samples,
            shutdown_tx: Some(shutdown_tx),
            thread_handle: Some(thread_handle),
            device_name: device_friendly_name,
        })
    }

    /// Get the device name being captured.
    pub fn source_name(&self) -> &str {
        &self.device_name
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
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        info!("System audio capture stopped");
    }
}

/// Run the WASAPI loopback capture loop.
fn run_capture_loop(
    device: wasapi::Device,
    samples: Arc<Mutex<Vec<f32>>>,
    shutdown_rx: mpsc::Receiver<()>,
) -> Result<(), SystemAudioError> {
    use wasapi::SampleType;

    // Create audio client for loopback capture
    let mut audio_client = device
        .get_iaudioclient()
        .map_err(|e| SystemAudioError::StreamFailed(e.to_string()))?;

    // Get the device format
    let device_format = device
        .get_device_format()
        .map_err(|e| SystemAudioError::StreamFailed(e.to_string()))?;

    let device_sample_rate = device_format.get_samplespersec();
    let device_channels = device_format.get_nchannels();

    debug!(
        "Device format: {} Hz, {} channels",
        device_sample_rate, device_channels
    );

    // Initialize for loopback capture (shared mode with events)
    audio_client
        .initialize_client(&device_format, &Direction::Capture, &ShareMode::Shared)
        .map_err(|e| SystemAudioError::StreamFailed(e.to_string()))?;

    // Get capture client
    let capture_client = audio_client
        .get_audiocaptureclient()
        .map_err(|e| SystemAudioError::StreamFailed(e.to_string()))?;

    // Start capture
    audio_client
        .start_stream()
        .map_err(|e| SystemAudioError::CaptureError(e.to_string()))?;

    let resampler_ratio = SAMPLE_RATE as f32 / device_sample_rate as f32;
    let mut capture_buffer: VecDeque<u8> = VecDeque::new();

    loop {
        // Check for shutdown signal
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        // Read available data into the deque
        match capture_client.read_from_device_to_deque(&mut capture_buffer) {
            Ok(_buffer_info) => {
                if !capture_buffer.is_empty() {
                    // Convert bytes to f32 samples based on format
                    let float_samples = bytes_to_f32_samples(&capture_buffer, &device_format);
                    capture_buffer.clear();

                    // Mix to mono if stereo
                    let mono_samples = if device_channels == 2 {
                        float_samples
                            .chunks(2)
                            .map(|chunk| {
                                if chunk.len() == 2 {
                                    (chunk[0] + chunk[1]) / 2.0
                                } else {
                                    chunk[0]
                                }
                            })
                            .collect::<Vec<_>>()
                    } else {
                        float_samples
                    };

                    // Simple linear resampling to 16kHz
                    let resampled = resample_linear(&mono_samples, resampler_ratio);

                    // Add to buffer
                    let mut buffer = samples.lock().unwrap();
                    buffer.extend(resampled);
                }
            }
            Err(e) => {
                warn!("Capture read error: {:?}", e);
            }
        }

        // Small sleep to prevent busy loop
        thread::sleep(std::time::Duration::from_millis(10));
    }

    audio_client
        .stop_stream()
        .map_err(|e| SystemAudioError::CaptureError(e.to_string()))?;

    Ok(())
}

/// Convert raw bytes to f32 samples based on wave format.
fn bytes_to_f32_samples(bytes: &VecDeque<u8>, format: &WaveFormat) -> Vec<f32> {
    let bytes_vec: Vec<u8> = bytes.iter().copied().collect();
    let bits_per_sample = format.get_bitspersample();

    match bits_per_sample {
        16 => {
            // 16-bit signed integer
            bytes_vec
                .chunks_exact(2)
                .map(|chunk| {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    sample as f32 / i16::MAX as f32
                })
                .collect()
        }
        24 => {
            // 24-bit signed integer
            bytes_vec
                .chunks_exact(3)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                    sample as f32 / (1 << 23) as f32
                })
                .collect()
        }
        32 => {
            // 32-bit float or int
            if format.get_validbitspersample() == 32 {
                // Float
                bytes_vec
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect()
            } else {
                // Integer
                bytes_vec
                    .chunks_exact(4)
                    .map(|chunk| {
                        let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        sample as f32 / i32::MAX as f32
                    })
                    .collect()
            }
        }
        _ => {
            warn!("Unsupported bit depth: {}", bits_per_sample);
            Vec::new()
        }
    }
}

/// Simple linear resampling.
fn resample_linear(samples: &[f32], ratio: f32) -> Vec<f32> {
    if (ratio - 1.0).abs() < f32::EPSILON {
        return samples.to_vec();
    }

    let new_len = (samples.len() as f32 * ratio) as usize;
    let mut output = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f32 / ratio;
        let idx_floor = src_idx.floor() as usize;
        let frac = src_idx - idx_floor as f32;

        let sample = if idx_floor + 1 < samples.len() {
            samples[idx_floor] * (1.0 - frac) + samples[idx_floor + 1] * frac
        } else if idx_floor < samples.len() {
            samples[idx_floor]
        } else {
            0.0
        };

        output.push(sample);
    }

    output
}

/// List available audio output devices (for loopback capture).
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    wasapi::initialize_mta().map_err(|e| SystemAudioError::InitFailed(e.to_string()))?;

    let devices = wasapi::DeviceCollection::new(&Direction::Render)
        .map_err(|e| SystemAudioError::InitFailed(e.to_string()))?;

    let mut sources = Vec::new();
    let count = devices.get_nbr_devices().unwrap_or(0);

    for i in 0..count {
        if let Ok(device) = devices.get_device_at_index(i) {
            let name = device.get_id().unwrap_or_else(|_| format!("device-{}", i));
            let description = device
                .get_friendlyname()
                .unwrap_or_else(|_| "Unknown Device".to_string());

            // Get device format for sample rate and channels
            let (sample_rate, channels) = if let Ok(format) = device.get_device_format() {
                (format.get_samplespersec(), format.get_nchannels() as u8)
            } else {
                (48000, 2)
            };

            sources.push(SourceInfo {
                name,
                description: format!("{} (Loopback)", description),
                is_monitor: true,
                sample_rate,
                channels,
            });
        }
    }

    Ok(sources)
}

/// Check if WASAPI loopback is available.
pub fn is_available() -> bool {
    wasapi::initialize_mta().is_ok()
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
    fn test_resample_linear() {
        let samples = vec![0.0, 1.0, 0.0, -1.0];
        let resampled = resample_linear(&samples, 0.5);
        assert_eq!(resampled.len(), 2);
    }
}
