//! M2M-100 neural translation engine.
//!
//! M2M-100 is a multilingual encoder-decoder model that can translate
//! directly between 100 languages without relying on English as an
//! intermediate language.
//!
//! Model sizes:
//! - M2M-100 418M: ~1.5GB, good quality, fast
//! - M2M-100 1.2B: ~4.5GB, excellent quality, slower
//!
//! This implementation is a placeholder. Full implementation options:
//! 1. Use rust-bert crate with ONNX feature
//! 2. Use ort directly with rust-tokenizers for M2M-100 tokenization
//! 3. Use CTranslate2 via FFI
//!
//! For now, use the Ollama backend for translation.

// Allow dead_code since this module is not yet fully implemented
#![allow(dead_code)]

use super::{is_m2m100_language, TranslationEngine, TranslationError};
use std::path::PathBuf;
use thiserror::Error;

/// M2M-100 specific errors.
#[derive(Error, Debug)]
pub enum M2M100Error {
    #[error("Model not found at {0}")]
    ModelNotFound(PathBuf),

    #[error("Model not downloaded. Run: openhush translation download m2m100-418m")]
    ModelNotDownloaded,

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Feature not yet implemented")]
    NotImplemented,
}

/// M2M-100 model variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum M2M100Model {
    /// 418M parameter model (~1.5GB)
    #[default]
    Small,
    /// 1.2B parameter model (~4.5GB)
    Large,
}

impl M2M100Model {
    /// Get the model name for downloads.
    pub fn name(&self) -> &str {
        match self {
            Self::Small => "m2m100-418m",
            Self::Large => "m2m100-1.2b",
        }
    }

    /// Get the Hugging Face model ID.
    pub fn hf_model_id(&self) -> &str {
        match self {
            Self::Small => "facebook/m2m100_418M",
            Self::Large => "facebook/m2m100_1.2B",
        }
    }

    /// Get the ONNX model URL (pre-converted).
    pub fn onnx_url(&self) -> &str {
        match self {
            Self::Small => "https://huggingface.co/optimum/m2m100_418M/resolve/main/",
            Self::Large => {
                // 1.2B needs to be converted - not pre-available
                "https://huggingface.co/facebook/m2m100_1.2B/"
            }
        }
    }

    /// Estimated VRAM usage in MB.
    pub fn vram_mb(&self) -> u32 {
        match self {
            Self::Small => 1500,
            Self::Large => 4500,
        }
    }
}

impl std::str::FromStr for M2M100Model {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "418m" | "small" | "m2m100-418m" => Ok(Self::Small),
            "1.2b" | "large" | "m2m100-1.2b" => Ok(Self::Large),
            _ => Err(format!("Unknown M2M-100 model: {}", s)),
        }
    }
}

/// M2M-100 translation engine.
///
/// Currently a placeholder - returns NotImplemented error.
/// Use OllamaTranslator for actual translation until M2M-100 is implemented.
pub struct M2M100Engine {
    model: M2M100Model,
    model_dir: PathBuf,
    loaded: bool,
}

impl M2M100Engine {
    /// Create a new M2M-100 engine.
    pub fn new(model: M2M100Model, model_dir: PathBuf) -> Self {
        Self {
            model,
            model_dir,
            loaded: false,
        }
    }

    /// Check if the model files exist.
    pub fn is_downloaded(&self) -> bool {
        let encoder_path = self.model_dir.join("encoder_model.onnx");
        let decoder_path = self.model_dir.join("decoder_model.onnx");
        let tokenizer_path = self.model_dir.join("sentencepiece.bpe.model");

        encoder_path.exists() && decoder_path.exists() && tokenizer_path.exists()
    }

    /// Get the model directory path.
    pub fn model_dir(&self) -> &PathBuf {
        &self.model_dir
    }

    /// Get the model variant.
    pub fn model(&self) -> M2M100Model {
        self.model
    }

    /// Load the model into memory.
    ///
    /// TODO: Implement actual model loading using ort + rust-tokenizers
    pub fn load(&mut self) -> Result<(), M2M100Error> {
        if !self.is_downloaded() {
            return Err(M2M100Error::ModelNotDownloaded);
        }

        // TODO: Load ONNX encoder and decoder models
        // TODO: Load sentencepiece tokenizer

        Err(M2M100Error::NotImplemented)
    }

    /// Unload the model from memory.
    pub fn unload(&mut self) {
        self.loaded = false;
        // TODO: Drop model resources
    }
}

impl TranslationEngine for M2M100Engine {
    async fn translate(
        &self,
        _text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        // Validate languages
        if !is_m2m100_language(from) {
            return Err(TranslationError::M2M100(M2M100Error::UnsupportedLanguage(
                from.to_string(),
            )));
        }
        if !is_m2m100_language(to) {
            return Err(TranslationError::M2M100(M2M100Error::UnsupportedLanguage(
                to.to_string(),
            )));
        }

        if !self.loaded {
            return Err(TranslationError::ModelNotLoaded);
        }

        // TODO: Implement actual translation
        // 1. Tokenize input with source language token
        // 2. Run encoder
        // 3. Run decoder autoregressively with target language token
        // 4. Detokenize output

        Err(TranslationError::M2M100(M2M100Error::NotImplemented))
    }

    fn supports_pair(&self, from: &str, to: &str) -> bool {
        is_m2m100_language(from) && is_m2m100_language(to)
    }

    fn name(&self) -> &str {
        "m2m100"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m2m100_model_from_str() {
        assert_eq!("418m".parse::<M2M100Model>().unwrap(), M2M100Model::Small);
        assert_eq!("1.2b".parse::<M2M100Model>().unwrap(), M2M100Model::Large);
        assert!("invalid".parse::<M2M100Model>().is_err());
    }

    #[test]
    fn test_m2m100_model_name() {
        assert_eq!(M2M100Model::Small.name(), "m2m100-418m");
        assert_eq!(M2M100Model::Large.name(), "m2m100-1.2b");
    }

    #[test]
    fn test_m2m100_vram() {
        assert_eq!(M2M100Model::Small.vram_mb(), 1500);
        assert_eq!(M2M100Model::Large.vram_mb(), 4500);
    }

    #[test]
    fn test_m2m100_engine_supports_pair() {
        let engine = M2M100Engine::new(M2M100Model::Small, PathBuf::from("/tmp/m2m100"));
        assert!(engine.supports_pair("en", "de"));
        assert!(engine.supports_pair("zh", "fr"));
        assert!(!engine.supports_pair("xyz", "en"));
    }

    #[test]
    fn test_m2m100_error_display() {
        let err = M2M100Error::ModelNotDownloaded;
        assert!(err.to_string().contains("openhush translation download"));
    }
}
