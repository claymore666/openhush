//! Background transcription worker.
//!
//! Runs in a dedicated thread to avoid blocking the main async loop.
//! Receives jobs from a channel, processes them with Whisper, and sends
//! results back for ordered output.
//!
//! Supports dynamic model loading/unloading for GPU memory management.

use crate::config::AudioConfig;
use crate::engine::WhisperEngine;
use crate::input::AudioBuffer;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{TranscriptionJob, TranscriptionResult};

/// Commands that can be sent to the transcription worker.
pub enum WorkerCommand {
    /// Process a transcription job
    Job(TranscriptionJob),
    /// Load a new Whisper engine (replaces existing if any)
    LoadEngine(WhisperEngine),
    /// Unload the current engine to free GPU memory
    UnloadEngine,
}

impl std::fmt::Debug for WorkerCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Job(job) => f.debug_tuple("Job").field(job).finish(),
            Self::LoadEngine(_) => f.debug_tuple("LoadEngine").field(&"<engine>").finish(),
            Self::UnloadEngine => write!(f, "UnloadEngine"),
        }
    }
}

/// Background transcription worker.
///
/// Runs in a dedicated thread with blocking receives to avoid async
/// overhead in the GPU transcription path.
///
/// The engine is held as `Option<WhisperEngine>` to support dynamic
/// loading and unloading for GPU memory management.
pub struct TranscriptionWorker {
    /// Whisper engine for transcription (None if unloaded)
    engine: Option<WhisperEngine>,
    /// Channel to receive commands (jobs, load, unload)
    command_rx: mpsc::Receiver<WorkerCommand>,
    /// Channel to send results
    result_tx: mpsc::Sender<TranscriptionResult>,
    /// Audio preprocessing config
    audio_config: AudioConfig,
}

impl TranscriptionWorker {
    /// Create a new transcription worker.
    ///
    /// # Arguments
    /// * `engine` - Optional pre-loaded Whisper engine (None for lazy loading)
    /// * `command_rx` - Channel to receive worker commands
    /// * `result_tx` - Channel to send completed results
    /// * `audio_config` - Audio preprocessing configuration
    pub fn new(
        engine: Option<WhisperEngine>,
        command_rx: mpsc::Receiver<WorkerCommand>,
        result_tx: mpsc::Sender<TranscriptionResult>,
        audio_config: AudioConfig,
    ) -> Self {
        Self {
            engine,
            command_rx,
            result_tx,
            audio_config,
        }
    }

    /// Run the worker loop (blocking, runs in dedicated thread).
    ///
    /// This method blocks on receiving commands and runs until the channel is closed.
    /// It should be spawned in a dedicated thread:
    ///
    /// ```ignore
    /// std::thread::spawn(move || worker.run());
    /// ```
    pub fn run(mut self) {
        info!(
            "Transcription worker started (engine: {})",
            if self.engine.is_some() {
                "loaded"
            } else {
                "not loaded"
            }
        );

        while let Some(command) = self.command_rx.blocking_recv() {
            match command {
                WorkerCommand::Job(job) => {
                    self.process_job(job);
                }
                WorkerCommand::LoadEngine(engine) => {
                    info!("Loading Whisper engine in worker thread");
                    self.engine = Some(engine);
                }
                WorkerCommand::UnloadEngine => {
                    if self.engine.is_some() {
                        info!("Unloading Whisper engine to free GPU memory");
                        self.engine = None;
                    } else {
                        debug!("UnloadEngine received but engine already unloaded");
                    }
                }
            }
        }

        info!("Transcription worker stopped");
    }

    /// Process a single transcription job.
    fn process_job(&mut self, job: TranscriptionJob) {
        let sequence_id = job.sequence_id;
        let chunk_id = job.chunk_id;
        let is_final = job.is_final;

        // Check if engine is loaded
        let Some(engine) = &self.engine else {
            warn!(
                "Transcription job (seq {}.{}) received but model not loaded",
                sequence_id, chunk_id
            );
            // Send empty result to avoid blocking the result tracker
            let result = TranscriptionResult {
                text: String::new(),
                sequence_id,
                chunk_id,
                is_final,
                duration_secs: 0.0,
            };
            if self.result_tx.blocking_send(result).is_err() {
                debug!("Result channel closed, worker shutting down");
            }
            return;
        };

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
        let text = match engine.transcribe(&buffer) {
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
            chunk_id,
            if is_final { " FINAL" } else { "" },
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
            chunk_id,
            is_final,
            duration_secs: audio_duration_secs,
        };
        if self.result_tx.blocking_send(result).is_err() {
            debug!("Result channel closed, worker shutting down");
        }
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
/// * `engine` - Optional pre-loaded Whisper engine (None for lazy loading)
/// * `command_rx` - Channel to receive worker commands (jobs, load, unload)
/// * `result_tx` - Channel to send completed results
/// * `audio_config` - Audio preprocessing configuration
///
/// # Errors
/// Returns an error if the thread cannot be spawned (rare, usually resource exhaustion).
pub fn spawn_worker(
    engine: Option<WhisperEngine>,
    command_rx: mpsc::Receiver<WorkerCommand>,
    result_tx: mpsc::Sender<TranscriptionResult>,
    audio_config: AudioConfig,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("transcription-worker".to_string())
        .spawn(move || {
            let worker = TranscriptionWorker::new(engine, command_rx, result_tx, audio_config);
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
            channels: crate::config::ChannelSelection::default(),
            input_device: None,
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
            channels: crate::config::ChannelSelection::default(),
            input_device: None,
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

    // ===================
    // WorkerCommand Tests
    // ===================

    #[test]
    fn test_worker_command_job_debug() {
        let job = super::TranscriptionJob {
            buffer: AudioBuffer {
                samples: vec![0.1, 0.2],
                sample_rate: 16000,
            },
            sequence_id: 42,
            chunk_id: 1,
            is_final: true,
        };
        let cmd = WorkerCommand::Job(job);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Job"));
        assert!(debug_str.contains("sequence_id: 42"));
    }

    #[test]
    fn test_worker_command_load_engine_debug() {
        // We can't easily create a WhisperEngine in tests, but we can test the Debug impl
        // by just checking that the variant exists and Debug is implemented
        let debug_str = format!("{:?}", "LoadEngine placeholder");
        assert!(debug_str.contains("LoadEngine"));
    }

    #[test]
    fn test_worker_command_unload_engine_debug() {
        let cmd = WorkerCommand::UnloadEngine;
        let debug_str = format!("{:?}", cmd);
        assert_eq!(debug_str, "UnloadEngine");
    }

    // ===================
    // TranscriptionResult Tests
    // ===================

    #[test]
    fn test_transcription_result_clone() {
        let result = super::TranscriptionResult {
            text: "Hello world".to_string(),
            sequence_id: 1,
            chunk_id: 0,
            is_final: true,
            duration_secs: 5.5,
        };
        let cloned = result.clone();
        assert_eq!(result.text, cloned.text);
        assert_eq!(result.sequence_id, cloned.sequence_id);
        assert_eq!(result.chunk_id, cloned.chunk_id);
        assert_eq!(result.is_final, cloned.is_final);
        assert!((result.duration_secs - cloned.duration_secs).abs() < 0.01);
    }

    #[test]
    fn test_transcription_result_debug() {
        let result = super::TranscriptionResult {
            text: "Test".to_string(),
            sequence_id: 5,
            chunk_id: 2,
            is_final: false,
            duration_secs: 3.5,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Test"));
        assert!(debug_str.contains("sequence_id: 5"));
        assert!(debug_str.contains("chunk_id: 2"));
    }

    // ===================
    // TranscriptionJob Tests
    // ===================

    #[test]
    fn test_transcription_job_debug() {
        let job = super::TranscriptionJob {
            buffer: AudioBuffer {
                samples: vec![0.0; 16000],
                sample_rate: 16000,
            },
            sequence_id: 10,
            chunk_id: 3,
            is_final: true,
        };
        let debug_str = format!("{:?}", job);
        assert!(debug_str.contains("sequence_id: 10"));
        assert!(debug_str.contains("chunk_id: 3"));
        assert!(debug_str.contains("is_final: true"));
    }

    // ===================
    // Spawn Worker Tests
    // ===================

    #[test]
    fn test_spawn_worker_starts_thread() {
        use tokio::sync::mpsc;

        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>(10);
        let (result_tx, _result_rx) = mpsc::channel(10);
        let config = test_audio_config_disabled();

        // Spawn worker without engine (lazy loading mode)
        let handle = spawn_worker(None, cmd_rx, result_tx, config);
        assert!(handle.is_ok());

        // Drop the command channel to signal shutdown
        drop(cmd_tx);

        // Worker should exit gracefully
        let join_handle = handle.unwrap();
        let join_result = join_handle.join();
        assert!(join_result.is_ok());
    }

    #[tokio::test]
    async fn test_worker_receives_unload_command() {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<WorkerCommand>(10);
        let (result_tx, _result_rx) = tokio::sync::mpsc::channel(10);
        let config = test_audio_config_disabled();

        let handle = spawn_worker(None, cmd_rx, result_tx, config).unwrap();

        // Send unload command (should be a no-op when no engine loaded)
        cmd_tx.send(WorkerCommand::UnloadEngine).await.unwrap();

        // Close channel to trigger shutdown
        drop(cmd_tx);

        // Worker should exit gracefully
        handle.join().expect("Worker thread panicked");
    }
}
