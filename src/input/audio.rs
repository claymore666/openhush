//! Audio capture using cpal.
//!
//! Captures audio from the microphone at 16kHz mono for Whisper compatibility.
//!
//! Supports two modes:
//! - **Legacy mode**: `start()`/`stop()` for backward compatibility
//! - **Always-on mode**: `new_always_on()` + `mark()`/`extract()` for instant capture

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleRate, Stream, StreamConfig};
use nnnoiseless::DenoiseState;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use super::ring_buffer::{AudioMark, AudioRingBuffer};
use crate::config::ResamplingQuality;

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// Minimum recording duration in seconds (filters accidental taps)
pub const MIN_DURATION_SECS: f32 = 0.1;

/// Minimum audio duration for Whisper (1000ms)
/// Audio shorter than this will be padded with silence
/// We use 1.1s to account for resampling/rounding errors
pub const WHISPER_MIN_DURATION_SECS: f32 = 1.1;

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

    #[allow(dead_code)]
    #[error("Not currently recording")]
    NotRecording,

    #[allow(dead_code)]
    #[error("Already recording")]
    AlreadyRecording,

    #[allow(dead_code)]
    #[error("Audio device disconnected")]
    DeviceDisconnected,

    #[allow(dead_code)]
    #[error("Stream error: {0}")]
    StreamError(String),
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
    #[allow(dead_code)]
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

    /// Calculate RMS (Root Mean Square) level in dB
    pub fn rms_db(&self) -> f32 {
        if self.samples.is_empty() {
            return f32::NEG_INFINITY;
        }

        let sum_squares: f32 = self.samples.iter().map(|&s| s * s).sum();
        let rms = (sum_squares / self.samples.len() as f32).sqrt();

        if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            f32::NEG_INFINITY
        }
    }

    /// Normalize audio to target RMS level in dB
    ///
    /// RMS normalization is better suited for speech than peak normalization
    /// as it considers the average loudness rather than just the peaks.
    pub fn normalize_rms(&mut self, target_db: f32) {
        let current_rms_db = self.rms_db();

        if current_rms_db.is_finite() {
            let gain_db = target_db - current_rms_db;
            self.apply_gain(gain_db);
            debug!(
                "RMS normalized: {:.1} dB -> {:.1} dB (gain: {:.1} dB)",
                current_rms_db, target_db, gain_db
            );
        } else {
            debug!("Skipping normalization: audio is silent");
        }
    }

    /// Apply gain in dB to all samples
    pub fn apply_gain(&mut self, gain_db: f32) {
        let gain_linear = 10.0_f32.powf(gain_db / 20.0);

        for sample in &mut self.samples {
            *sample *= gain_linear;
        }
    }

    /// Apply dynamic compression with attack/release envelope
    ///
    /// - `threshold_db`: Level where compression kicks in
    /// - `ratio`: Compression ratio (e.g., 4.0 for 4:1)
    /// - `attack_ms`: Time to reach full compression
    /// - `release_ms`: Time to release compression
    /// - `makeup_gain_db`: Gain applied after compression
    pub fn compress(
        &mut self,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    ) {
        if self.samples.is_empty() || ratio <= 1.0 {
            return;
        }

        let threshold_linear = 10.0_f32.powf(threshold_db / 20.0);

        // Calculate time constants (samples)
        let attack_coeff = (-1.0 / (attack_ms * self.sample_rate as f32 / 1000.0)).exp();
        let release_coeff = (-1.0 / (release_ms * self.sample_rate as f32 / 1000.0)).exp();

        let mut envelope = 0.0_f32;

        let rms_before = self.rms_db();

        for sample in &mut self.samples {
            let input_abs = sample.abs();

            // Smooth envelope follower with attack/release
            if input_abs > envelope {
                envelope = attack_coeff * envelope + (1.0 - attack_coeff) * input_abs;
            } else {
                envelope = release_coeff * envelope + (1.0 - release_coeff) * input_abs;
            }

            // Calculate gain reduction
            let gain = if envelope > threshold_linear {
                let over_db = 20.0 * (envelope / threshold_linear).log10();
                let compressed_db = over_db / ratio;
                let reduction_db = over_db - compressed_db;
                10.0_f32.powf(-reduction_db / 20.0)
            } else {
                1.0
            };

            *sample *= gain;
        }

        // Apply makeup gain
        if makeup_gain_db != 0.0 {
            self.apply_gain(makeup_gain_db);
        }

        let rms_after = self.rms_db();
        debug!(
            "Compressed: {:.1} dB -> {:.1} dB (ratio: {}:1, makeup: {:.1} dB)",
            rms_before, rms_after, ratio, makeup_gain_db
        );
    }

    /// Apply limiter to prevent clipping
    ///
    /// - `ceiling_db`: Maximum output level (e.g., -1.0 dB)
    /// - `release_ms`: Release time for the limiter
    pub fn limit(&mut self, ceiling_db: f32, release_ms: f32) {
        if self.samples.is_empty() {
            return;
        }

        let ceiling_linear = 10.0_f32.powf(ceiling_db / 20.0);
        let release_coeff = (-1.0 / (release_ms * self.sample_rate as f32 / 1000.0)).exp();

        let mut gain_reduction = 1.0_f32;
        let mut peaks_limited = 0_usize;

        for sample in &mut self.samples {
            let input_abs = sample.abs();

            // Calculate required gain reduction for this sample
            let target_gain = if input_abs > ceiling_linear {
                peaks_limited += 1;
                ceiling_linear / input_abs
            } else {
                1.0
            };

            // Instant attack (brick wall), smooth release
            if target_gain < gain_reduction {
                gain_reduction = target_gain; // Instant attack
            } else {
                gain_reduction =
                    release_coeff * gain_reduction + (1.0 - release_coeff) * target_gain;
            }

            *sample *= gain_reduction;
        }

        if peaks_limited > 0 {
            debug!(
                "Limited {} samples to {:.1} dB ceiling",
                peaks_limited, ceiling_db
            );
        }
    }

    /// Apply RNNoise neural network noise reduction
    ///
    /// RNNoise is designed for real-time speech enhancement, removing
    /// background noise, keyboard clicks, fan noise, and other non-speech sounds.
    ///
    /// - `strength`: Mix between original (0.0) and denoised (1.0) audio
    ///
    /// Note: This internally resamples to 48kHz (RNNoise native rate) and back.
    pub fn denoise(&mut self, strength: f32) {
        if self.samples.is_empty() || strength <= 0.0 {
            return;
        }

        let strength = strength.clamp(0.0, 1.0);

        // RNNoise constants
        const RNNOISE_SAMPLE_RATE: u32 = 48000;
        const RNNOISE_FRAME_SIZE: usize = 480; // 10ms at 48kHz

        // Keep original for mixing
        let original_samples = if strength < 1.0 {
            Some(self.samples.clone())
        } else {
            None
        };

        // Resample 16kHz -> 48kHz for RNNoise
        let upsampled = if self.sample_rate != RNNOISE_SAMPLE_RATE {
            resample_for_rnnoise(&self.samples, self.sample_rate, RNNOISE_SAMPLE_RATE)
        } else {
            self.samples.clone()
        };

        // Process through RNNoise
        let mut denoiser = DenoiseState::new();
        let mut denoised = Vec::with_capacity(upsampled.len());

        // Scale from [-1.0, 1.0] to [-32768.0, 32767.0] for RNNoise
        let scaled: Vec<f32> = upsampled.iter().map(|&s| s * 32767.0).collect();

        // Process in 480-sample frames
        let mut frame_input = [0.0f32; RNNOISE_FRAME_SIZE];
        let mut frame_output = [0.0f32; RNNOISE_FRAME_SIZE];

        for (i, chunk) in scaled.chunks(RNNOISE_FRAME_SIZE).enumerate() {
            // Copy to frame buffer (pad with zeros if last chunk is short)
            frame_input[..chunk.len()].copy_from_slice(chunk);
            if chunk.len() < RNNOISE_FRAME_SIZE {
                frame_input[chunk.len()..].fill(0.0);
            }

            // Process frame
            let _vad_prob = denoiser.process_frame(&mut frame_output, &frame_input);

            // Skip first frame (contains fade-in artifacts) but still include
            // its output for correct alignment, just attenuate it
            if i == 0 {
                // Fade in the first frame to reduce artifacts
                for (j, sample) in frame_output.iter().enumerate() {
                    let fade = j as f32 / RNNOISE_FRAME_SIZE as f32;
                    denoised.push(sample * fade / 32767.0);
                }
            } else if chunk.len() < RNNOISE_FRAME_SIZE {
                // Last partial frame - only take what we need
                for &sample in &frame_output[..chunk.len()] {
                    denoised.push(sample / 32767.0);
                }
            } else {
                // Normal frame - scale back to [-1.0, 1.0]
                for &sample in &frame_output {
                    denoised.push(sample / 32767.0);
                }
            }
        }

        // Resample 48kHz -> 16kHz back to original rate
        let downsampled = if self.sample_rate != RNNOISE_SAMPLE_RATE {
            resample_for_rnnoise(&denoised, RNNOISE_SAMPLE_RATE, self.sample_rate)
        } else {
            denoised
        };

        // Ensure same length as original (resampling may cause slight differences)
        let original_len = self.samples.len();
        self.samples = if downsampled.len() >= original_len {
            downsampled[..original_len].to_vec()
        } else {
            let mut result = downsampled;
            result.resize(original_len, 0.0);
            result
        };

        // Mix with original if strength < 1.0
        if let Some(orig) = original_samples {
            for (i, sample) in self.samples.iter_mut().enumerate() {
                *sample = orig[i] * (1.0 - strength) + *sample * strength;
            }
        }

        debug!("Applied RNNoise denoising (strength: {:.2})", strength);
    }
}

/// Audio recorder for capturing microphone input
///
/// Supports two modes:
/// - **Legacy mode**: Create with `new()`, use `start()`/`stop()` for recording
/// - **Always-on mode**: Create with `new_always_on()`, use `mark()`/`extract()`
pub struct AudioRecorder {
    device: Device,
    config: StreamConfig,
    /// Legacy mode: recording flag
    recording: Arc<AtomicBool>,
    /// Legacy mode: sample buffer
    #[allow(dead_code)]
    samples: Arc<Mutex<Vec<f32>>>,
    /// Stream (always running in always-on mode)
    stream: Option<Stream>,
    device_sample_rate: u32,
    /// Always-on mode: ring buffer for continuous capture
    ring_buffer: Option<Arc<AudioRingBuffer>>,
    /// Always-on mode: whether stream is always running
    always_on: bool,
    /// Resampling quality setting
    resampling_quality: ResamplingQuality,
}

impl AudioRecorder {
    /// Create a new audio recorder using the default input device (legacy mode)
    ///
    /// In legacy mode, call `start()` to begin recording and `stop()` to end.
    #[allow(dead_code)]
    pub fn new() -> Result<Self, AudioRecorderError> {
        Self::new_internal(None, ResamplingQuality::default())
    }

    /// Create a new always-on audio recorder
    ///
    /// In always-on mode, the audio stream starts immediately and continuously
    /// fills a ring buffer. Use `mark()` when the hotkey is pressed and
    /// `extract()` when released for instant audio capture with no startup delay.
    ///
    /// # Arguments
    /// * `prebuffer_secs` - Duration of audio to buffer (default: 30.0 seconds)
    /// * `resampling_quality` - Quality of audio resampling (low=linear, high=sinc)
    pub fn new_always_on(
        prebuffer_secs: f32,
        resampling_quality: ResamplingQuality,
    ) -> Result<Self, AudioRecorderError> {
        Self::new_internal(Some(prebuffer_secs), resampling_quality)
    }

    /// Internal constructor
    fn new_internal(
        prebuffer_secs: Option<f32>,
        resampling_quality: ResamplingQuality,
    ) -> Result<Self, AudioRecorderError> {
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

        let always_on = prebuffer_secs.is_some();
        let ring_buffer = prebuffer_secs.map(|secs| {
            // Create ring buffer at device sample rate (we resample on extract)
            Arc::new(AudioRingBuffer::new(secs, device_sample_rate))
        });

        let mut recorder = Self {
            device,
            config,
            recording: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            device_sample_rate,
            ring_buffer,
            always_on,
            resampling_quality,
        };

        // In always-on mode, start the stream immediately
        if always_on {
            recorder.start_always_on_stream()?;
        }

        Ok(recorder)
    }

    /// Start the always-on stream (internal)
    fn start_always_on_stream(&mut self) -> Result<(), AudioRecorderError> {
        let ring_buffer = self
            .ring_buffer
            .clone()
            .expect("ring buffer required for always-on mode");
        let err_fn = |err| error!("Audio stream error: {}", err);

        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    ring_buffer.push_samples(data);
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioRecorderError::StreamBuildFailed(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioRecorderError::StreamStartFailed(e.to_string()))?;

        self.stream = Some(stream);
        info!("Always-on audio stream started");

        Ok(())
    }

    /// List available audio input devices
    #[allow(dead_code)]
    pub fn list_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }

    /// Start recording audio
    #[allow(dead_code)]
    pub fn start(&mut self) -> Result<(), AudioRecorderError> {
        if self.recording.load(Ordering::SeqCst) {
            return Err(AudioRecorderError::AlreadyRecording);
        }

        // Clear previous samples
        {
            // Use unwrap_or_else to handle poisoned mutex (shouldn't happen, but be safe)
            let mut samples = self.samples.lock().unwrap_or_else(|poisoned| {
                warn!("Samples mutex was poisoned, recovering");
                poisoned.into_inner()
            });
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
                        // Use if let to silently skip on poisoned mutex (audio callback can't panic)
                        if let Ok(mut samples_guard) = samples.lock() {
                            samples_guard.extend_from_slice(data);
                        }
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
    #[allow(dead_code)]
    pub fn stop(&mut self) -> Result<AudioBuffer, AudioRecorderError> {
        if !self.recording.load(Ordering::SeqCst) {
            return Err(AudioRecorderError::NotRecording);
        }

        self.recording.store(false, Ordering::SeqCst);

        // Drop the stream to stop recording
        self.stream = None;

        // Get the recorded samples
        let samples = {
            let samples_guard = self.samples.lock().unwrap_or_else(|poisoned| {
                warn!("Samples mutex was poisoned, recovering");
                poisoned.into_inner()
            });
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
            resample(
                &samples,
                self.device_sample_rate,
                SAMPLE_RATE,
                self.resampling_quality,
            )
        } else {
            samples
        };

        let mut buffer = AudioBuffer {
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

        // Pad with silence if shorter than Whisper's minimum (1000ms)
        if buffer.duration_secs() < WHISPER_MIN_DURATION_SECS {
            let samples_needed = (SAMPLE_RATE as f32 * WHISPER_MIN_DURATION_SECS) as usize;
            let padding = samples_needed - buffer.samples.len();
            debug!(
                "Padding audio with {} samples of silence ({:.0}ms)",
                padding,
                padding as f32 / SAMPLE_RATE as f32 * 1000.0
            );
            buffer.samples.extend(vec![0.0f32; padding]);
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

    // ========================================================================
    // Always-on mode methods
    // ========================================================================

    /// Mark the current position in the ring buffer (always-on mode)
    ///
    /// Call this when the hotkey is pressed. The returned mark can be used
    /// with `extract()` when the hotkey is released.
    ///
    /// # Panics
    /// Panics if not in always-on mode (use `new_always_on()` to create).
    pub fn mark(&self) -> AudioMark {
        let ring_buffer = self
            .ring_buffer
            .as_ref()
            .expect("mark() requires always-on mode (use new_always_on())");

        let mark = ring_buffer.mark();
        debug!("Marked audio position (sequence_id: {})", mark.sequence_id);
        mark
    }

    /// Extract audio from the mark position to now (always-on mode)
    ///
    /// Call this when the hotkey is released. Returns all audio recorded
    /// since the mark was created.
    ///
    /// # Arguments
    /// * `mark` - The mark created when recording started
    ///
    /// # Returns
    /// AudioBuffer with resampled audio at 16kHz, or error if too short.
    #[allow(dead_code)]
    pub fn extract(&self, mark: &AudioMark) -> Result<AudioBuffer, AudioRecorderError> {
        let ring_buffer = self
            .ring_buffer
            .as_ref()
            .expect("extract() requires always-on mode (use new_always_on())");

        let samples = ring_buffer.extract_since(mark);
        let duration_secs = samples.len() as f32 / self.device_sample_rate as f32;

        debug!(
            "Extracted {:.2}s of audio from ring buffer (sequence_id: {})",
            duration_secs, mark.sequence_id
        );

        // Check minimum duration
        if duration_secs < MIN_DURATION_SECS {
            warn!(
                "Recording too short: {:.2}s (minimum {:.2}s)",
                duration_secs, MIN_DURATION_SECS
            );
            return Err(AudioRecorderError::TooShort);
        }

        // Resample to 16kHz if needed
        let resampled = if self.device_sample_rate != SAMPLE_RATE {
            resample(
                &samples,
                self.device_sample_rate,
                SAMPLE_RATE,
                self.resampling_quality,
            )
        } else {
            samples
        };

        let mut buffer = AudioBuffer {
            samples: resampled,
            sample_rate: SAMPLE_RATE,
        };

        // Pad with silence if shorter than Whisper's minimum (1000ms)
        if buffer.duration_secs() < WHISPER_MIN_DURATION_SECS {
            let samples_needed = (SAMPLE_RATE as f32 * WHISPER_MIN_DURATION_SECS) as usize;
            let padding = samples_needed - buffer.samples.len();
            debug!(
                "Padding audio with {} samples of silence ({:.0}ms)",
                padding,
                padding as f32 / SAMPLE_RATE as f32 * 1000.0
            );
            buffer.samples.extend(vec![0.0f32; padding]);
        }

        info!(
            "Extracted audio: {:.2}s ({} samples, sequence_id: {})",
            buffer.duration_secs(),
            buffer.samples.len(),
            mark.sequence_id
        );

        Ok(buffer)
    }

    /// Shutdown the always-on stream
    ///
    /// Call this when the daemon is shutting down to release audio resources.
    #[allow(dead_code)]
    pub fn shutdown(&mut self) {
        if self.always_on {
            self.stream = None;
            info!("Always-on audio stream stopped");
        }
    }

    /// Check if in always-on mode
    #[allow(dead_code)]
    pub fn is_always_on(&self) -> bool {
        self.always_on
    }

    /// Get access to the ring buffer (for wake word detection, etc.)
    #[allow(dead_code)]
    pub fn ring_buffer(&self) -> Option<&Arc<AudioRingBuffer>> {
        self.ring_buffer.as_ref()
    }

    /// Get the current position in the ring buffer (always-on mode)
    ///
    /// Used for streaming chunk extraction. Save this position after
    /// extracting a chunk to know where to start the next one.
    pub fn current_position(&self) -> usize {
        let ring_buffer = self
            .ring_buffer
            .as_ref()
            .expect("current_position() requires always-on mode");

        ring_buffer.current_position()
    }

    /// Extract a chunk of audio from one position to another (always-on mode)
    ///
    /// Used for streaming chunk extraction during recording. Call this
    /// periodically (e.g., every 5 seconds) to get chunks for transcription.
    ///
    /// # Arguments
    /// * `from_pos` - Start position (from mark or previous chunk)
    /// * `to_pos` - End position (usually current_position())
    ///
    /// # Returns
    /// AudioBuffer with resampled audio at 16kHz, or None if too short.
    pub fn extract_chunk(&self, from_pos: usize, to_pos: usize) -> Option<AudioBuffer> {
        let ring_buffer = self
            .ring_buffer
            .as_ref()
            .expect("extract_chunk() requires always-on mode");

        let samples = ring_buffer.extract_range(from_pos, to_pos);
        let duration_secs = samples.len() as f32 / self.device_sample_rate as f32;

        if samples.is_empty() || duration_secs < MIN_DURATION_SECS {
            debug!(
                "Chunk too short: {:.2}s (minimum {:.2}s)",
                duration_secs, MIN_DURATION_SECS
            );
            return None;
        }

        // Resample to 16kHz if needed
        let resampled = if self.device_sample_rate != SAMPLE_RATE {
            resample(
                &samples,
                self.device_sample_rate,
                SAMPLE_RATE,
                self.resampling_quality,
            )
        } else {
            samples
        };

        let mut buffer = AudioBuffer {
            samples: resampled,
            sample_rate: SAMPLE_RATE,
        };

        // Pad with silence if shorter than Whisper's minimum (1000ms)
        if buffer.duration_secs() < WHISPER_MIN_DURATION_SECS {
            let samples_needed = (SAMPLE_RATE as f32 * WHISPER_MIN_DURATION_SECS) as usize;
            let padding = samples_needed - buffer.samples.len();
            debug!(
                "Padding chunk with {} samples of silence ({:.0}ms)",
                padding,
                padding as f32 / SAMPLE_RATE as f32 * 1000.0
            );
            buffer.samples.extend(vec![0.0f32; padding]);
        }

        debug!(
            "Extracted chunk: {:.2}s ({} samples)",
            buffer.duration_secs(),
            buffer.samples.len()
        );

        Some(buffer)
    }

    /// Check if the current audio device is still available
    ///
    /// Returns true if the device is available, false if disconnected.
    #[allow(dead_code)]
    pub fn is_device_available(&self) -> bool {
        // Try to get the device name - if this fails, device is likely disconnected
        match self.device.name() {
            Ok(_) => {
                // Also check if we can still get a config
                self.device.default_input_config().is_ok()
            }
            Err(_) => false,
        }
    }

    /// Get the current device name
    #[allow(dead_code)]
    pub fn device_name(&self) -> String {
        self.device.name().unwrap_or_else(|_| "unknown".to_string())
    }

    /// Try to reinitialize with a new default device
    ///
    /// Call this when the current device is disconnected and a new one becomes available.
    /// Returns the new device name on success.
    #[allow(dead_code)]
    pub fn try_reinitialize(&mut self) -> Result<String, AudioRecorderError> {
        info!("Attempting to reinitialize audio capture...");

        // Stop the current stream
        self.stream = None;

        let host = cpal::default_host();

        // Try to get a new default device
        let device = host
            .default_input_device()
            .ok_or(AudioRecorderError::NoInputDevice)?;

        let device_name = device.name().unwrap_or_else(|_| "unknown".to_string());
        info!("Found new audio device: {}", device_name);

        // Get new config
        let supported_config = device
            .default_input_config()
            .map_err(|e| AudioRecorderError::NoInputConfig(e.to_string()))?;

        let device_sample_rate = supported_config.sample_rate().0;
        info!(
            "Device sample rate: {} Hz (will resample to {} Hz)",
            device_sample_rate, SAMPLE_RATE
        );

        // Build new config
        let config = StreamConfig {
            channels: 1,
            sample_rate: SampleRate(device_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        // Update fields
        self.device = device;
        self.config = config;
        self.device_sample_rate = device_sample_rate;

        // If always-on mode, create new ring buffer and start stream
        if self.always_on {
            // Get prebuffer duration from existing ring buffer
            let prebuffer_secs = self
                .ring_buffer
                .as_ref()
                .map(|rb| rb.duration_secs())
                .unwrap_or(30.0);

            // Create new ring buffer at new device sample rate
            self.ring_buffer = Some(Arc::new(AudioRingBuffer::new(
                prebuffer_secs,
                device_sample_rate,
            )));

            // Start the new stream
            self.start_always_on_stream()?;
            info!("Audio stream reinitialized successfully");
        }

        Ok(device_name)
    }
}

/// Check if any audio input device is available
#[allow(dead_code)]
pub fn has_input_device() -> bool {
    let host = cpal::default_host();
    host.default_input_device().is_some()
}

/// Get the name of the default input device, if any
#[allow(dead_code)]
pub fn default_device_name() -> Option<String> {
    let host = cpal::default_host();
    host.default_input_device().and_then(|d| d.name().ok())
}

/// Resample audio using the specified quality setting
///
/// - `Low`: Fast linear interpolation (lower quality)
/// - `High`: Sinc interpolation via rubato (higher quality, better for transcription)
fn resample(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
    quality: ResamplingQuality,
) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    match quality {
        ResamplingQuality::Low => resample_linear(samples, from_rate, to_rate),
        ResamplingQuality::High => resample_sinc(samples, from_rate, to_rate),
    }
}

/// Simple linear resampling (fast, lower quality)
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut result = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let src_idx_floor = src_idx.floor() as usize;
        let src_idx_ceil = (src_idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - src_idx_floor as f64;

        // Linear interpolation
        let sample =
            samples[src_idx_floor] * (1.0 - frac as f32) + samples[src_idx_ceil] * frac as f32;
        result.push(sample);
    }

    result
}

/// Fast linear resampling for RNNoise (16kHz <-> 48kHz)
///
/// Uses simple linear interpolation which is sufficient for
/// internal RNNoise processing where we're doing 16kHz -> 48kHz -> 16kHz.
fn resample_for_rnnoise(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    resample_linear(samples, from_rate, to_rate)
}

/// High-quality sinc resampling via rubato
///
/// Uses polyphase sinc interpolation which is the standard for professional audio.
/// This provides better frequency response and less aliasing than linear interpolation.
fn resample_sinc(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    // Configure sinc resampler for high quality audio
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let resample_ratio = to_rate as f64 / from_rate as f64;

    // Create resampler for mono audio (1 channel)
    // chunk_size is how many input samples we process at once
    let chunk_size = 1024;
    let mut resampler = match SincFixedIn::<f32>::new(
        resample_ratio,
        2.0, // max relative ratio (allows some flexibility)
        params,
        chunk_size,
        1, // mono
    ) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to create sinc resampler: {}, falling back to linear", e);
            return resample_linear(samples, from_rate, to_rate);
        }
    };

    let mut output = Vec::with_capacity((samples.len() as f64 * resample_ratio) as usize + 1024);

    // Process in chunks
    let mut pos = 0;
    while pos < samples.len() {
        let end = (pos + chunk_size).min(samples.len());
        let chunk = &samples[pos..end];

        // Pad last chunk if needed
        let input_chunk: Vec<f32> = if chunk.len() < chunk_size {
            let mut padded = chunk.to_vec();
            padded.resize(chunk_size, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        // rubato expects Vec<Vec<f32>> for multi-channel, we have mono
        let input_frames = vec![input_chunk];

        match resampler.process(&input_frames, None) {
            Ok(resampled) => {
                if !resampled.is_empty() && !resampled[0].is_empty() {
                    // For the last chunk, only take the proportional amount
                    if chunk.len() < chunk_size {
                        let expected_out = (chunk.len() as f64 * resample_ratio).ceil() as usize;
                        let take = expected_out.min(resampled[0].len());
                        output.extend_from_slice(&resampled[0][..take]);
                    } else {
                        output.extend_from_slice(&resampled[0]);
                    }
                }
            }
            Err(e) => {
                warn!("Sinc resampling error: {}, falling back to linear", e);
                return resample_linear(samples, from_rate, to_rate);
            }
        }

        pos = end;
    }

    debug!(
        "Sinc resampled {} -> {} samples ({}Hz -> {}Hz)",
        samples.len(),
        output.len(),
        from_rate,
        to_rate
    );

    output
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
            samples: vec![0.0; 800], // 0.05s at 16kHz (below 0.1s minimum)
            sample_rate: 16000,
        };
        assert!(!short_buffer.is_valid());

        let valid_buffer = AudioBuffer {
            samples: vec![0.0; 1600], // 0.1s at 16kHz (exactly at minimum)
            sample_rate: 16000,
        };
        assert!(valid_buffer.is_valid());
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let result = resample(&samples, 16000, 16000, ResamplingQuality::Low);
        assert_eq!(result, samples);
    }

    #[test]
    fn test_resample_downsample_linear() {
        let samples: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let result = resample(&samples, 48000, 16000, ResamplingQuality::Low);
        assert!(result.len() < samples.len());
    }

    #[test]
    fn test_resample_downsample_sinc() {
        let samples: Vec<f32> = (0..4800).map(|i| i as f32).collect();
        let result = resample(&samples, 48000, 16000, ResamplingQuality::High);
        assert!(result.len() < samples.len());
        // Sinc resampling should produce roughly 1/3 the samples (48kHz -> 16kHz)
        let expected_len = (samples.len() as f64 * 16000.0 / 48000.0) as usize;
        assert!((result.len() as i32 - expected_len as i32).abs() < 100);
    }

    #[test]
    fn test_rms_db_silence() {
        let buffer = AudioBuffer {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
        };
        assert!(buffer.rms_db().is_infinite());
    }

    #[test]
    fn test_rms_db_full_scale() {
        // Full scale sine wave has RMS of 1/sqrt(2) ≈ 0.707, which is ~-3 dB
        let samples: Vec<f32> = (0..16000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();
        let buffer = AudioBuffer {
            samples,
            sample_rate: 16000,
        };
        let rms = buffer.rms_db();
        // RMS of sine wave is -3.01 dB
        assert!((rms - (-3.01)).abs() < 0.1);
    }

    #[test]
    fn test_normalize_rms() {
        // Create a quiet signal (approx -40 dB RMS)
        let samples: Vec<f32> = (0..16000)
            .map(|i| 0.01 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();
        let mut buffer = AudioBuffer {
            samples,
            sample_rate: 16000,
        };

        let target_db = -18.0;
        buffer.normalize_rms(target_db);

        let rms_after = buffer.rms_db();
        assert!((rms_after - target_db).abs() < 0.5);
    }

    #[test]
    fn test_apply_gain() {
        let mut buffer = AudioBuffer {
            samples: vec![0.5, -0.5, 0.25, -0.25],
            sample_rate: 16000,
        };

        // +6 dB doubles amplitude
        buffer.apply_gain(6.02); // 20 * log10(2) ≈ 6.02 dB

        assert!((buffer.samples[0] - 1.0).abs() < 0.01);
        assert!((buffer.samples[1] - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn test_compress_reduces_dynamic_range() {
        // Create signal with loud and quiet parts
        let mut samples = Vec::with_capacity(32000);
        // First half: loud (0.8 amplitude)
        for i in 0..16000 {
            samples.push(0.8 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin());
        }
        // Second half: quiet (0.1 amplitude)
        for i in 0..16000 {
            samples.push(0.1 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin());
        }

        let mut buffer = AudioBuffer {
            samples,
            sample_rate: 16000,
        };

        // Get peak levels before compression
        let loud_peak_before = buffer.samples[..16000]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        let quiet_peak_before = buffer.samples[16000..]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);

        // Compress with 4:1 ratio and makeup gain
        buffer.compress(-20.0, 4.0, 5.0, 50.0, 0.0);

        let loud_peak_after = buffer.samples[..16000]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        let quiet_peak_after = buffer.samples[16000..]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);

        // Loud section should be reduced
        assert!(loud_peak_after < loud_peak_before);
        // Dynamic range should be reduced (ratio of loud/quiet should be smaller)
        let ratio_before = loud_peak_before / quiet_peak_before;
        let ratio_after = loud_peak_after / quiet_peak_after;
        assert!(ratio_after < ratio_before);
    }

    #[test]
    fn test_limiter_prevents_clipping() {
        // Create signal with peaks above 1.0
        let mut buffer = AudioBuffer {
            samples: vec![0.5, 1.5, -1.2, 0.8, 2.0, -0.3],
            sample_rate: 16000,
        };

        // Limit to -1 dB (ceiling ≈ 0.89)
        buffer.limit(-1.0, 50.0);

        let ceiling = 10.0_f32.powf(-1.0 / 20.0); // ~0.89
        for sample in &buffer.samples {
            assert!(
                sample.abs() <= ceiling + 0.01,
                "Sample {} exceeds ceiling {}",
                sample,
                ceiling
            );
        }
    }

    #[test]
    fn test_limiter_preserves_quiet_audio() {
        let original = vec![0.1, -0.2, 0.15, -0.05];
        let mut buffer = AudioBuffer {
            samples: original.clone(),
            sample_rate: 16000,
        };

        buffer.limit(-1.0, 50.0);

        // Quiet audio should be unchanged
        for (orig, processed) in original.iter().zip(buffer.samples.iter()) {
            assert!((orig - processed).abs() < 0.001);
        }
    }

    #[test]
    fn test_denoise_processes_audio() {
        // Create a 1 second buffer at 16kHz with some noise-like content
        let samples: Vec<f32> = (0..16000)
            .map(|i| {
                // Mix of speech-like frequency (300Hz) and noise (random-ish)
                let speech = (2.0 * std::f32::consts::PI * 300.0 * i as f32 / 16000.0).sin() * 0.5;
                let noise = ((i * 7) % 100) as f32 / 100.0 - 0.5;
                (speech + noise * 0.1).clamp(-1.0, 1.0)
            })
            .collect();

        let mut buffer = AudioBuffer {
            samples: samples.clone(),
            sample_rate: 16000,
        };

        buffer.denoise(1.0);

        // After denoising, buffer should still have same length
        assert_eq!(buffer.samples.len(), samples.len());

        // Samples should be within valid range
        for sample in &buffer.samples {
            assert!(
                *sample >= -1.0 && *sample <= 1.0,
                "Sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_denoise_strength_mixing() {
        let samples: Vec<f32> = (0..16000).map(|i| (i as f32 / 16000.0) * 0.5).collect();
        let mut buffer = AudioBuffer {
            samples: samples.clone(),
            sample_rate: 16000,
        };

        // With strength 0, audio should be unchanged
        buffer.denoise(0.0);
        for (orig, processed) in samples.iter().zip(buffer.samples.iter()) {
            assert!((orig - processed).abs() < 0.001);
        }
    }

    #[test]
    fn test_denoise_empty_buffer() {
        let mut buffer = AudioBuffer {
            samples: vec![],
            sample_rate: 16000,
        };

        // Should not panic on empty buffer
        buffer.denoise(1.0);
        assert!(buffer.samples.is_empty());
    }
}
