//! Background transcription worker.
//!
//! Runs in a dedicated thread to avoid blocking the main async loop.
//! Receives jobs from a channel, processes them with Whisper, and sends
//! results back for ordered output.

use crate::config::AudioConfig;
use crate::engine::WhisperEngine;
use crate::input::AudioBuffer;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::{TranscriptionJob, TranscriptionResult};

/// Background transcription worker.
///
/// Runs in a dedicated thread with blocking receives to avoid async
/// overhead in the GPU transcription path.
pub struct TranscriptionWorker {
    /// Whisper engine for transcription
    engine: WhisperEngine,
    /// Channel to receive jobs
    job_rx: mpsc::Receiver<TranscriptionJob>,
    /// Channel to send results
    result_tx: mpsc::Sender<TranscriptionResult>,
    /// Audio preprocessing config
    audio_config: AudioConfig,
}

impl TranscriptionWorker {
    /// Create a new transcription worker.
    ///
    /// # Arguments
    /// * `engine` - Pre-loaded Whisper engine
    /// * `job_rx` - Channel to receive transcription jobs
    /// * `result_tx` - Channel to send completed results
    /// * `audio_config` - Audio preprocessing configuration
    pub fn new(
        engine: WhisperEngine,
        job_rx: mpsc::Receiver<TranscriptionJob>,
        result_tx: mpsc::Sender<TranscriptionResult>,
        audio_config: AudioConfig,
    ) -> Self {
        Self {
            engine,
            job_rx,
            result_tx,
            audio_config,
        }
    }

    /// Run the worker loop (blocking, runs in dedicated thread).
    ///
    /// This method blocks on receiving jobs and runs until the channel is closed.
    /// It should be spawned in a dedicated thread:
    ///
    /// ```ignore
    /// std::thread::spawn(move || worker.run());
    /// ```
    pub fn run(mut self) {
        info!("Transcription worker started");

        while let Some(job) = self.job_rx.blocking_recv() {
            let sequence_id = job.sequence_id;
            let total_start = std::time::Instant::now();
            debug!(
                "Processing transcription job (sequence_id: {})",
                sequence_id
            );

            // Preprocess audio
            let preprocess_start = std::time::Instant::now();
            let mut buffer = job.buffer;
            let audio_duration_secs = buffer.duration_secs();
            Self::preprocess_audio(&mut buffer, &self.audio_config);
            let preprocess_ms = preprocess_start.elapsed().as_millis();

            // Transcribe
            let transcribe_start = std::time::Instant::now();
            let text = match self.engine.transcribe(&buffer) {
                Ok(result) => result.text,
                Err(e) => {
                    error!("Transcription failed (sequence_id: {}): {}", sequence_id, e);
                    String::new()
                }
            };
            let transcribe_ms = transcribe_start.elapsed().as_millis();
            let total_ms = total_start.elapsed().as_millis();

            // Log timing breakdown
            info!(
                "⏱️  Timing (seq {}.{}{}): audio={:.1}s | preprocess={}ms | transcribe={}ms | total={}ms | ratio={:.2}x",
                sequence_id,
                job.chunk_id,
                if job.is_final { " FINAL" } else { "" },
                audio_duration_secs,
                preprocess_ms,
                transcribe_ms,
                total_ms,
                total_ms as f32 / (audio_duration_secs * 1000.0)
            );

            // Send result
            let result = TranscriptionResult {
                text,
                sequence_id,
                chunk_id: job.chunk_id,
                is_final: job.is_final,
            };
            if self.result_tx.blocking_send(result).is_err() {
                debug!("Result channel closed, worker shutting down");
                break;
            }
        }

        info!("Transcription worker stopped");
    }

    /// Apply audio preprocessing (noise reduction, normalization, compression, limiter).
    fn preprocess_audio(buffer: &mut AudioBuffer, config: &AudioConfig) {
        // Noise reduction is independent of the preprocessing flag
        // as it's a separate feature that can be enabled standalone
        if config.noise_reduction.enabled {
            debug!(
                "Applying RNNoise noise reduction (strength: {:.2})",
                config.noise_reduction.strength
            );
            buffer.denoise(config.noise_reduction.strength);
        }

        if !config.preprocessing {
            return;
        }

        let rms_before = buffer.rms_db();
        debug!("Preprocessing audio (input RMS: {:.1} dB)", rms_before);

        // 1. RMS Normalization
        if config.normalization.enabled {
            buffer.normalize_rms(config.normalization.target_db);
        }

        // 2. Dynamic Compression
        if config.compression.enabled {
            buffer.compress(
                config.compression.threshold_db,
                config.compression.ratio,
                config.compression.attack_ms,
                config.compression.release_ms,
                config.compression.makeup_gain_db,
            );
        }

        // 3. Limiter (safety net)
        if config.limiter.enabled {
            buffer.limit(config.limiter.ceiling_db, config.limiter.release_ms);
        }

        let rms_after = buffer.rms_db();
        info!(
            "Audio preprocessed: {:.1} dB -> {:.1} dB",
            rms_before, rms_after
        );
    }
}

/// Spawn a transcription worker in a dedicated thread.
///
/// Returns a handle to the thread for optional join on shutdown.
///
/// # Arguments
/// * `engine` - Pre-loaded Whisper engine
/// * `job_rx` - Channel to receive transcription jobs
/// * `result_tx` - Channel to send completed results
/// * `audio_config` - Audio preprocessing configuration
///
/// # Errors
/// Returns an error if the thread cannot be spawned (rare, usually resource exhaustion).
pub fn spawn_worker(
    engine: WhisperEngine,
    job_rx: mpsc::Receiver<TranscriptionJob>,
    result_tx: mpsc::Sender<TranscriptionResult>,
    audio_config: AudioConfig,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("transcription-worker".to_string())
        .spawn(move || {
            let worker = TranscriptionWorker::new(engine, job_rx, result_tx, audio_config);
            worker.run();
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // Audio Preprocessing Tests
    // ===================

    fn test_audio_config_disabled() -> AudioConfig {
        AudioConfig {
            prebuffer_duration_secs: 30.0,
            resampling_quality: crate::config::ResamplingQuality::High,
            preprocessing: false,
            normalization: crate::config::NormalizationConfig::default(),
            compression: crate::config::CompressionConfig::default(),
            limiter: crate::config::LimiterConfig::default(),
            noise_reduction: crate::config::NoiseReductionConfig::default(),
        }
    }

    fn test_audio_config_enabled() -> AudioConfig {
        AudioConfig {
            prebuffer_duration_secs: 30.0,
            resampling_quality: crate::config::ResamplingQuality::High,
            preprocessing: true,
            normalization: crate::config::NormalizationConfig {
                enabled: true,
                target_db: -18.0,
            },
            compression: crate::config::CompressionConfig {
                enabled: true,
                threshold_db: -24.0,
                ratio: 4.0,
                attack_ms: 5.0,
                release_ms: 50.0,
                makeup_gain_db: 6.0,
            },
            limiter: crate::config::LimiterConfig {
                enabled: true,
                ceiling_db: -1.0,
                release_ms: 50.0,
            },
            noise_reduction: crate::config::NoiseReductionConfig::default(),
        }
    }

    #[test]
    fn test_preprocess_audio_disabled() {
        let config = test_audio_config_disabled();
        let mut buffer = AudioBuffer {
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            sample_rate: 16000,
        };
        let original = buffer.samples.clone();

        TranscriptionWorker::preprocess_audio(&mut buffer, &config);

        // With preprocessing disabled, samples should be unchanged
        assert_eq!(buffer.samples, original);
    }

    #[test]
    fn test_preprocess_audio_enabled() {
        let config = test_audio_config_enabled();
        let mut buffer = AudioBuffer {
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            sample_rate: 16000,
        };
        let original = buffer.samples.clone();

        TranscriptionWorker::preprocess_audio(&mut buffer, &config);

        // With preprocessing enabled, samples should be modified
        // (normalization, compression, limiting)
        assert_ne!(buffer.samples, original);
    }

    #[test]
    fn test_preprocess_audio_with_noise_reduction() {
        let mut config = test_audio_config_disabled();
        config.noise_reduction.enabled = true;
        config.noise_reduction.strength = 0.5;

        let mut buffer = AudioBuffer {
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            sample_rate: 16000,
        };

        // This should run without panicking
        TranscriptionWorker::preprocess_audio(&mut buffer, &config);
    }

    #[test]
    fn test_preprocess_audio_empty_buffer() {
        let config = test_audio_config_enabled();
        let mut buffer = AudioBuffer {
            samples: vec![],
            sample_rate: 16000,
        };

        // Should not panic on empty buffer
        TranscriptionWorker::preprocess_audio(&mut buffer, &config);
        assert!(buffer.samples.is_empty());
    }

    #[test]
    fn test_preprocess_audio_silence() {
        let config = test_audio_config_enabled();
        let mut buffer = AudioBuffer {
            samples: vec![0.0; 1000],
            sample_rate: 16000,
        };

        // Should not panic on silence
        TranscriptionWorker::preprocess_audio(&mut buffer, &config);
    }

    #[test]
    fn test_preprocess_audio_preserves_sample_rate() {
        let config = test_audio_config_enabled();
        let mut buffer = AudioBuffer {
            samples: vec![0.1, 0.2, 0.3],
            sample_rate: 16000,
        };

        TranscriptionWorker::preprocess_audio(&mut buffer, &config);

        // Sample rate should be unchanged
        assert_eq!(buffer.sample_rate, 16000);
    }
}
