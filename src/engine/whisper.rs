//! Whisper transcription engine using whisper-rs.

use crate::config::Config;
use crate::input::AudioBuffer;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Error, Debug)]
pub enum WhisperError {
    #[error("Model not found at {0}. Run 'openhush model download {1}'")]
    ModelNotFound(PathBuf, String),

    #[error("Failed to load model: {0}")]
    LoadFailed(String),

    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),

    #[error("Invalid audio: {0}")]
    InvalidAudio(String),
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

/// Whisper transcription engine
pub struct WhisperEngine {
    ctx: WhisperContext,
    language: String,
}

impl WhisperEngine {
    /// Create a new Whisper engine, loading the model from disk
    pub fn new(model_path: &Path, language: &str) -> Result<Self, WhisperError> {
        info!("Loading Whisper model from: {}", model_path.display());

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

        let params = WhisperContextParameters::default();

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap_or_default(),
            params,
        )
        .map_err(|e| WhisperError::LoadFailed(format!("{:?}", e)))?;

        info!("Whisper model loaded successfully");

        Ok(Self {
            ctx,
            language: language.to_string(),
        })
    }

    /// Load engine from config
    #[allow(dead_code)]
    pub fn from_config(config: &Config) -> Result<Self, WhisperError> {
        let data_dir = Config::data_dir()
            .map_err(|e| WhisperError::LoadFailed(e.to_string()))?;

        let model = WhisperModel::from_str(&config.transcription.model)
            .unwrap_or(WhisperModel::Base);

        let model_path = data_dir.join("models").join(model.filename());

        Self::new(&model_path, &config.transcription.language)
    }

    /// Transcribe audio buffer to text
    pub fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult, WhisperError> {
        if audio.samples.is_empty() {
            return Err(WhisperError::InvalidAudio("Empty audio buffer".into()));
        }

        let start_time = std::time::Instant::now();

        debug!(
            "Transcribing {:.2}s of audio ({} samples)",
            audio.duration_secs(),
            audio.samples.len()
        );

        // Create transcription state
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| WhisperError::TranscriptionFailed(format!("{:?}", e)))?;

        // Configure parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language
        if self.language != "auto" {
            params.set_language(Some(&self.language));
        }

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
        let num_segments = state.full_n_segments()
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

/// Get the model directory path
#[allow(dead_code)]
pub fn models_dir() -> Result<PathBuf, WhisperError> {
    let data_dir = Config::data_dir()
        .map_err(|e| WhisperError::LoadFailed(e.to_string()))?;
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
        assert_eq!(WhisperModel::from_str("LARGE-V3"), Some(WhisperModel::LargeV3));
        assert_eq!(WhisperModel::from_str("invalid"), None);
    }

    #[test]
    fn test_model_filename() {
        assert_eq!(WhisperModel::Tiny.filename(), "ggml-tiny.bin");
        assert_eq!(WhisperModel::LargeV3.filename(), "ggml-large-v3.bin");
    }
}
