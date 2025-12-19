//! Whisper transcription engine using whisper-rs.

use crate::config::Config;
use crate::engine::validation::{self, AudioValidationError};
use crate::input::AudioBuffer;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

#[derive(Error, Debug)]
pub enum WhisperError {
    #[error("Model not found at {0}. Run 'openhush model download {1}'")]
    ModelNotFound(PathBuf, String),

    #[error("Failed to load model: {0}")]
    LoadFailed(String),

    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),

    #[error("Audio validation failed: {0}")]
    ValidationFailed(#[from] AudioValidationError),
}

/// Result of transcription
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Language detected or used
    #[allow(dead_code)]
    pub language: String,
    /// Processing time in milliseconds
    #[allow(dead_code)]
    pub duration_ms: u64,
}

/// Available Whisper models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    LargeV3,
}

impl WhisperModel {
    /// Parse model name from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tiny" => Some(Self::Tiny),
            "base" => Some(Self::Base),
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" | "large-v3" | "largev3" => Some(Self::LargeV3),
            _ => None,
        }
    }

    /// Get the model filename
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::Base => "ggml-base.bin",
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
            Self::LargeV3 => "ggml-large-v3.bin",
        }
    }

    /// Get model size in bytes (approximate)
    #[allow(dead_code)]
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Tiny => 75_000_000,
            Self::Base => 142_000_000,
            Self::Small => 466_000_000,
            Self::Medium => 1_500_000_000,
            Self::LargeV3 => 3_000_000_000,
        }
    }

    /// Get Hugging Face download URL
    #[allow(dead_code)]
    pub fn download_url(&self) -> String {
        let filename = self.filename();
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            filename
        )
    }
}

/// Whisper transcription engine with cached state for fast inference
pub struct WhisperEngine {
    /// Cached state for reuse across transcriptions (avoids GPU buffer reallocation)
    state: RefCell<WhisperState>,
    language: String,
    translate: bool,
}

impl WhisperEngine {
    /// Create a new Whisper engine, loading the model from disk
    ///
    /// If `translate` is true, all audio will be translated to English.
    /// If false, audio will be transcribed in its original language.
    ///
    /// The engine pre-allocates GPU buffers for fast transcription.
    pub fn new(
        model_path: &Path,
        language: &str,
        translate: bool,
        use_gpu: bool,
    ) -> Result<Self, WhisperError> {
        info!(
            "Loading Whisper model from: {} (GPU: {})",
            model_path.display(),
            use_gpu
        );

        if !model_path.exists() {
            // Extract model name from path for error message
            let model_name = model_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .strip_prefix("ggml-")
                .unwrap_or("unknown");

            return Err(WhisperError::ModelNotFound(
                model_path.to_path_buf(),
                model_name.to_string(),
            ));
        }

        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu);

        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap_or_default(), params)
            .map_err(|e| WhisperError::LoadFailed(format!("{:?}", e)))?;

        // Leak the context so the state can reference it for the daemon's lifetime
        let ctx: &'static WhisperContext = Box::leak(Box::new(ctx));

        // Pre-create state to allocate GPU buffers once
        info!("Pre-allocating GPU buffers...");
        let state = ctx
            .create_state()
            .map_err(|e| WhisperError::LoadFailed(format!("Failed to create state: {:?}", e)))?;

        info!("Whisper model loaded and GPU buffers allocated");

        Ok(Self {
            state: RefCell::new(state),
            language: language.to_string(),
            translate,
        })
    }

    /// Load engine from config
    #[allow(dead_code)]
    pub fn from_config(config: &Config) -> Result<Self, WhisperError> {
        let data_dir = Config::data_dir().map_err(|e| WhisperError::LoadFailed(e.to_string()))?;

        let model =
            WhisperModel::from_str(&config.transcription.model).unwrap_or(WhisperModel::Base);

        let model_path = data_dir.join("models").join(model.filename());
        let use_gpu = config.transcription.device.to_lowercase() != "cpu";

        Self::new(
            &model_path,
            &config.transcription.language,
            config.transcription.translate,
            use_gpu,
        )
    }

    /// Transcribe audio buffer to text
    pub fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult, WhisperError> {
        // Validate audio before FFI boundary
        let validation_info = validation::validate_audio(&audio.samples, audio.sample_rate)?;

        debug!(
            "Audio validated: {:.2}s, {} samples, RMS: {:.4}, range: [{:.3}, {:.3}]",
            validation_info.duration_secs,
            validation_info.sample_count,
            validation_info.rms,
            validation_info.min_value,
            validation_info.max_value
        );

        // Warn if audio levels seem unusual
        if validation_info.rms < 0.001 {
            warn!(
                "Audio appears to be silence or very quiet (RMS: {:.6})",
                validation_info.rms
            );
        }
        if validation_info.max_value.abs() > 1.0 || validation_info.min_value.abs() > 1.0 {
            warn!(
                "Audio samples outside normal range [-1, 1]: min={:.3}, max={:.3}",
                validation_info.min_value, validation_info.max_value
            );
        }

        let start_time = std::time::Instant::now();

        debug!(
            "Transcribing {:.2}s of audio ({} samples)",
            audio.duration_secs(),
            audio.samples.len()
        );

        // Use cached state (GPU buffers already allocated)
        let mut state = self.state.borrow_mut();

        // Configure parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language
        if self.language != "auto" {
            params.set_language(Some(&self.language));
        }

        // Set translate mode (true = translate to English, false = transcribe in original language)
        debug!("Setting translate={}", self.translate);
        params.set_translate(self.translate);

        // Disable printing to avoid cluttering output
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Run inference
        state
            .full(params, &audio.samples)
            .map_err(|e| WhisperError::TranscriptionFailed(format!("{:?}", e)))?;

        // Collect results
        let num_segments = state
            .full_n_segments()
            .map_err(|e| WhisperError::TranscriptionFailed(format!("{:?}", e)))?;

        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }

        // Trim whitespace
        let text = text.trim().to_string();

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Detect language (if auto)
        let detected_lang = if self.language == "auto" {
            // whisper-rs doesn't expose detected language easily
            // For now, just report "auto"
            "auto".to_string()
        } else {
            self.language.clone()
        };

        info!(
            "Transcription complete ({} chars, {}ms)",
            text.len(),
            duration_ms
        );

        Ok(TranscriptionResult {
            text,
            language: detected_lang,
            duration_ms,
        })
    }
}

/// Result of GPU benchmark
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Fixed overhead in seconds (time to process ~2s of audio)
    pub overhead_secs: f32,
    /// Recommended minimum chunk interval in seconds
    pub recommended_chunk_interval: f32,
    /// Audio duration used for benchmark
    #[allow(dead_code)]
    pub test_audio_secs: f32,
}

impl WhisperEngine {
    /// Benchmark GPU transcription to determine optimal chunk interval.
    ///
    /// Transcribes a short silence buffer to measure the fixed overhead
    /// of the transcription pipeline. This overhead is relatively constant
    /// regardless of audio length, so it determines the minimum viable
    /// chunk size for streaming.
    ///
    /// Returns the measured overhead and recommended chunk interval.
    pub fn benchmark(&self, safety_margin: f32) -> Result<BenchmarkResult, WhisperError> {
        use crate::input::AudioBuffer;

        info!("Running GPU benchmark to determine optimal chunk interval...");

        // Generate 2 seconds of silence for benchmarking
        // This is enough to trigger full pipeline but short enough to be fast
        let test_duration_secs = 2.0f32;
        let sample_rate = 16000;
        let num_samples = (test_duration_secs * sample_rate as f32) as usize;

        // Create silence buffer (zeros)
        let samples: Vec<f32> = vec![0.0; num_samples];
        let audio = AudioBuffer {
            samples,
            sample_rate,
        };

        // Warm-up run (first run may have additional JIT overhead)
        let _ = self.transcribe(&audio);

        // Benchmark run (average of 3 runs for stability)
        let mut total_ms: u64 = 0;
        const BENCHMARK_RUNS: u32 = 3;

        for i in 0..BENCHMARK_RUNS {
            let start = std::time::Instant::now();
            let _ = self.transcribe(&audio);
            let elapsed = start.elapsed().as_millis() as u64;
            total_ms += elapsed;
            debug!("Benchmark run {}: {}ms", i + 1, elapsed);
        }

        let avg_ms = total_ms / BENCHMARK_RUNS as u64;
        let overhead_secs = avg_ms as f32 / 1000.0;

        // Calculate recommended chunk interval:
        // min_chunk = overhead * (1 + safety_margin)
        // This ensures chunks complete before the next one is ready
        let recommended = overhead_secs * (1.0 + safety_margin);

        info!(
            "Benchmark complete: {:.2}s overhead, recommended chunk interval: {:.2}s (with {:.0}% margin)",
            overhead_secs,
            recommended,
            safety_margin * 100.0
        );

        Ok(BenchmarkResult {
            overhead_secs,
            recommended_chunk_interval: recommended,
            test_audio_secs: test_duration_secs,
        })
    }
}

/// Get the model directory path
#[allow(dead_code)]
pub fn models_dir() -> Result<PathBuf, WhisperError> {
    let data_dir = Config::data_dir().map_err(|e| WhisperError::LoadFailed(e.to_string()))?;
    Ok(data_dir.join("models"))
}

/// Check if a model is downloaded
#[allow(dead_code)]
pub fn is_model_downloaded(model: WhisperModel) -> bool {
    if let Ok(dir) = models_dir() {
        dir.join(model.filename()).exists()
    } else {
        false
    }
}

/// List downloaded models
#[allow(dead_code)]
pub fn list_downloaded_models() -> Vec<WhisperModel> {
    let all_models = [
        WhisperModel::Tiny,
        WhisperModel::Base,
        WhisperModel::Small,
        WhisperModel::Medium,
        WhisperModel::LargeV3,
    ];

    all_models
        .iter()
        .filter(|m| is_model_downloaded(**m))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_from_str() {
        assert_eq!(WhisperModel::from_str("tiny"), Some(WhisperModel::Tiny));
        assert_eq!(
            WhisperModel::from_str("LARGE-V3"),
            Some(WhisperModel::LargeV3)
        );
        assert_eq!(WhisperModel::from_str("invalid"), None);
    }

    #[test]
    fn test_model_filename() {
        assert_eq!(WhisperModel::Tiny.filename(), "ggml-tiny.bin");
        assert_eq!(WhisperModel::LargeV3.filename(), "ggml-large-v3.bin");
    }
}
