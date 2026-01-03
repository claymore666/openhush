//! System audio capture using WASAPI loopback on Windows.
//!
//! Captures desktop audio (meetings, calls, media) via Windows Audio Session API.
//! Works on Windows Vista and later.
//!
//! ## How It Works
//!
//! WASAPI loopback captures what's being played on the speakers. This module:
//! 1. Gets the default render (output) device
//! 2. Initializes it for capture with the LOOPBACK flag (set automatically by wasapi crate)
//! 3. Reads audio samples and resamples to 16kHz mono for Whisper
//!
//! ## Permissions
//!
//! No special permissions required on Windows (unlike macOS).

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// Native capture sample rate (Windows typically uses 48kHz)
const NATIVE_SAMPLE_RATE: u32 = 48000;

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
pub struct SystemAudioCapture {
    /// Audio samples buffer (16kHz mono f32)
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
    /// Uses WASAPI loopback to capture what's playing on the speakers.
    pub fn new(device_name: Option<&str>) -> Result<Self, SystemAudioError> {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = Arc::clone(&samples);

        // Get device info for the name
        let sources = list_monitor_sources()?;
        let device_desc = if let Some(name) = device_name {
            sources
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.description.clone())
                .unwrap_or_else(|| name.to_string())
        } else {
            sources
                .first()
                .map(|s| s.description.clone())
                .unwrap_or_else(|| "Default Output".to_string())
        };

        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_capture_loop(samples_clone, shutdown_rx) {
                error!("WASAPI capture error: {}", e);
            }
        });

        info!("System audio capture started from: {}", device_desc);

        Ok(Self {
            samples,
            shutdown_tx: Some(shutdown_tx),
            thread_handle: Some(thread_handle),
            device_name: device_desc,
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

    /// Get the current buffer duration in seconds.
    pub fn buffer_duration_secs(&self) -> f32 {
        self.buffer_len() as f32 / SAMPLE_RATE as f32
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        info!("System audio capture stopped");
    }
}

/// Run the audio capture loop (called in a separate thread).
fn run_capture_loop(
    samples: Arc<Mutex<Vec<f32>>>,
    shutdown_rx: mpsc::Receiver<()>,
) -> Result<(), SystemAudioError> {
    // Initialize COM for this thread
    wasapi::initialize_mta()
        .ok()
        .map_err(|_| SystemAudioError::InitFailed("COM initialization failed".into()))?;

    // Get device enumerator
    let enumerator = DeviceEnumerator::new()
        .map_err(|e| SystemAudioError::InitFailed(format!("DeviceEnumerator: {:?}", e)))?;

    // Get default RENDER device (for loopback capture)
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|e| SystemAudioError::NoOutputDevice)?;

    let device_name = device
        .get_friendlyname()
        .unwrap_or_else(|_| "Unknown".to_string());
    info!("Capturing loopback from: {}", device_name);

    // Get audio client from the render device
    let mut audio_client = device
        .get_iaudioclient()
        .map_err(|e| SystemAudioError::StreamFailed(format!("AudioClient: {:?}", e)))?;

    // Request 32-bit float, stereo, 48kHz (common Windows format)
    // autoconvert will handle format conversion if needed
    let desired_format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        NATIVE_SAMPLE_RATE as usize,
        2,
        None,
    );
    let blockalign = desired_format.get_blockalign();
    debug!("Desired capture format: {:?}", desired_format);

    // Use PollingShared mode for loopback (event mode doesn't work reliably for loopback)
    // The wasapi crate automatically sets AUDCLNT_STREAMFLAGS_LOOPBACK when:
    // - device is Render and we initialize with Direction::Capture
    let mode = StreamMode::PollingShared {
        autoconvert: true,
        buffer_duration_hns: 200_000, // 20ms buffer in 100ns units
    };

    // Initialize for CAPTURE on a RENDER device = loopback mode
    audio_client
        .initialize_client(&desired_format, &Direction::Capture, &mode)
        .map_err(|e| SystemAudioError::StreamFailed(format!("Initialize: {:?}", e)))?;

    debug!("Initialized WASAPI loopback capture");

    // Get capture client
    let capture_client = audio_client
        .get_audiocaptureclient()
        .map_err(|e| SystemAudioError::StreamFailed(format!("CaptureClient: {:?}", e)))?;

    // Buffer for raw samples
    let mut sample_queue: VecDeque<u8> = VecDeque::with_capacity(blockalign as usize * 4800);

    // Start the stream
    audio_client
        .start_stream()
        .map_err(|e| SystemAudioError::StreamFailed(format!("Start: {:?}", e)))?;

    info!("WASAPI loopback capture started");

    // Resampler state for 48kHz stereo -> 16kHz mono
    let mut resampler = SimpleResampler::new(NATIVE_SAMPLE_RATE, SAMPLE_RATE, 2);

    // Main capture loop
    loop {
        // Check for shutdown
        if shutdown_rx.try_recv().is_ok() {
            debug!("Shutdown signal received");
            break;
        }

        // Read available samples
        match capture_client.read_from_device_to_deque(&mut sample_queue) {
            Ok(_) => {}
            Err(e) => {
                warn!("Capture read error: {:?}", e);
            }
        }

        // Process samples in the queue
        while sample_queue.len() >= blockalign as usize {
            // Extract one frame (stereo f32 = 8 bytes)
            let mut frame_bytes = [0u8; 8];
            for byte in &mut frame_bytes {
                *byte = sample_queue.pop_front().unwrap();
            }

            // Convert to f32 samples
            let left = f32::from_le_bytes([
                frame_bytes[0],
                frame_bytes[1],
                frame_bytes[2],
                frame_bytes[3],
            ]);
            let right = f32::from_le_bytes([
                frame_bytes[4],
                frame_bytes[5],
                frame_bytes[6],
                frame_bytes[7],
            ]);

            // Mix to mono and feed to resampler
            let mono = (left + right) * 0.5;
            if let Some(resampled) = resampler.process(mono) {
                if let Ok(mut buffer) = samples.lock() {
                    buffer.push(resampled);
                }
            }
        }

        // Small sleep to prevent busy-waiting (polling mode)
        thread::sleep(std::time::Duration::from_millis(5));
    }

    // Stop the stream
    audio_client.stop_stream().ok();

    Ok(())
}

/// Simple linear resampler from native rate to target rate.
///
/// Uses linear interpolation for simplicity. For production use,
/// consider using the rubato crate for higher quality resampling.
struct SimpleResampler {
    /// Source sample rate
    source_rate: u32,
    /// Target sample rate
    target_rate: u32,
    /// Accumulator for fractional sample position
    accumulator: f64,
    /// Ratio of source/target rates
    ratio: f64,
    /// Previous sample for interpolation
    prev_sample: f32,
}

impl SimpleResampler {
    fn new(source_rate: u32, target_rate: u32, _channels: u8) -> Self {
        Self {
            source_rate,
            target_rate,
            accumulator: 0.0,
            ratio: source_rate as f64 / target_rate as f64,
            prev_sample: 0.0,
        }
    }

    /// Process one input sample, returns output sample when ready.
    fn process(&mut self, sample: f32) -> Option<f32> {
        self.accumulator += 1.0;

        if self.accumulator >= self.ratio {
            self.accumulator -= self.ratio;
            // Linear interpolation
            let frac = self.accumulator as f32;
            let output = self.prev_sample * (1.0 - frac) + sample * frac;
            self.prev_sample = sample;
            Some(output)
        } else {
            self.prev_sample = sample;
            None
        }
    }
}

/// List available audio output devices (for loopback capture).
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    // Initialize COM if not already done
    wasapi::initialize_mta().ok().ok();

    let enumerator = DeviceEnumerator::new()
        .map_err(|e| SystemAudioError::InitFailed(format!("DeviceEnumerator: {:?}", e)))?;

    let devices = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| SystemAudioError::InitFailed(format!("DeviceCollection: {:?}", e)))?;

    let mut sources = Vec::new();

    for device_result in &devices {
        let device = match device_result {
            Ok(d) => d,
            Err(_) => continue,
        };
        let name = device.get_id().unwrap_or_default();
        let description = device
            .get_friendlyname()
            .unwrap_or_else(|_| "Unknown Device".to_string());

        // Get format info
        let (sample_rate, channels) = if let Ok(format) = device.get_device_format() {
            (format.get_samplespersec(), format.get_nchannels() as u8)
        } else {
            (48000, 2) // Default assumption
        };

        sources.push(SourceInfo {
            name,
            description,
            is_monitor: true, // All render devices can be used for loopback
            sample_rate,
            channels,
        });
    }

    debug!("Found {} audio output devices for loopback", sources.len());
    for source in &sources {
        debug!(
            "  - {} ({}Hz, {} ch)",
            source.description, source.sample_rate, source.channels
        );
    }

    Ok(sources)
}

/// Check if WASAPI loopback is available.
pub fn is_available() -> bool {
    // WASAPI loopback is available on Windows Vista and later
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
    fn test_audio_source_from_str_aliases() {
        assert_eq!(
            "microphone".parse::<AudioSource>().unwrap(),
            AudioSource::Microphone
        );
        assert_eq!(
            "system".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );
        assert_eq!(
            "desktop".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );
        assert_eq!("mix".parse::<AudioSource>().unwrap(), AudioSource::Both);
        assert_eq!("all".parse::<AudioSource>().unwrap(), AudioSource::Both);
    }

    #[test]
    fn test_audio_source_default() {
        assert_eq!(AudioSource::default(), AudioSource::Microphone);
    }

    #[test]
    fn test_simple_resampler() {
        // 48000 -> 16000 = 3:1 ratio
        let mut resampler = SimpleResampler::new(48000, 16000, 1);

        // Should output roughly 1 sample for every 3 input samples
        let mut output_count = 0;
        for i in 0..300 {
            let sample = (i as f32 / 300.0).sin();
            if resampler.process(sample).is_some() {
                output_count += 1;
            }
        }

        // Should be approximately 100 output samples (300 / 3)
        assert!(output_count >= 95 && output_count <= 105);
    }

    #[test]
    fn test_source_info_creation() {
        let info = SourceInfo {
            name: "test_device".to_string(),
            description: "Test Speakers".to_string(),
            is_monitor: true,
            sample_rate: 48000,
            channels: 2,
        };
        assert_eq!(info.name, "test_device");
        assert!(info.is_monitor);
    }
}
