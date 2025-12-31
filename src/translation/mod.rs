//! Translation module for multilingual support.
//!
//! Provides text-to-text translation using various backends:
//! - M2M-100: High-quality neural translation for 100 languages
//! - Ollama: LLM-based translation for flexible language pairs
//!
//! Note: Whisper's built-in `translate` option is handled in the transcription
//! engine itself (any language -> English only).

mod m2m100;
mod ollama;
mod sentence_buffer;

// M2M-100 exports
pub use m2m100::{
    download_model as download_m2m100_model, is_model_downloaded as is_m2m100_downloaded,
    model_dir as m2m100_model_dir, remove_model as remove_m2m100_model, M2M100Engine, M2M100Error,
    M2M100Model,
};
pub use ollama::OllamaTranslator;
pub use sentence_buffer::SentenceBuffer;

use std::sync::Arc;
use tracing::warn;

use std::fmt;
use thiserror::Error;

/// Translation-related errors.
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum TranslationError {
    #[error("M2M-100 error: {0}")]
    M2M100(#[from] M2M100Error),

    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("Unsupported language pair: {from} -> {to}")]
    UnsupportedLanguagePair { from: String, to: String },

    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Translation disabled")]
    Disabled,
}

/// Translation result.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TranslationResult {
    /// Original text
    pub original: String,
    /// Translated text
    pub translated: String,
    /// Source language code
    pub from: String,
    /// Target language code
    pub to: String,
}

/// Translation engine trait for different backends.
#[allow(dead_code)]
pub trait TranslationEngine: Send + Sync {
    /// Translate text from source to target language.
    fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> impl std::future::Future<Output = Result<String, TranslationError>> + Send;

    /// Check if a language pair is supported.
    fn supports_pair(&self, from: &str, to: &str) -> bool;

    /// Get the name of the translation engine.
    fn name(&self) -> &str;
}

/// Supported M2M-100 languages (100 languages).
/// ISO 639-1 codes where available, otherwise ISO 639-3.
#[allow(dead_code)]
pub const M2M100_LANGUAGES: &[&str] = &[
    "af", "am", "ar", "ast", "az", "ba", "be", "bg", "bn", "br", "bs", "ca", "ceb", "cs", "cy",
    "da", "de", "el", "en", "es", "et", "fa", "ff", "fi", "fr", "fy", "ga", "gd", "gl", "gu", "ha",
    "he", "hi", "hr", "ht", "hu", "hy", "id", "ig", "ilo", "is", "it", "ja", "jv", "ka", "kk",
    "km", "kn", "ko", "lb", "lg", "ln", "lo", "lt", "lv", "mg", "mk", "ml", "mn", "mr", "ms", "my",
    "ne", "nl", "no", "ns", "oc", "or", "pa", "pl", "ps", "pt", "ro", "ru", "sd", "si", "sk", "sl",
    "so", "sq", "sr", "ss", "su", "sv", "sw", "ta", "th", "tl", "tn", "tr", "uk", "ur", "uz", "vi",
    "wo", "xh", "yi", "yo", "zh", "zu",
];

/// Check if a language is supported by M2M-100.
#[allow(dead_code)]
pub fn is_m2m100_language(lang: &str) -> bool {
    M2M100_LANGUAGES.contains(&lang.to_lowercase().as_str())
}

/// Translation engine type for configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum TranslationEngineType {
    /// M2M-100 neural translation (high quality, local)
    #[default]
    M2M100,
    /// Ollama LLM-based translation (flexible, local)
    Ollama,
}

impl fmt::Display for TranslationEngineType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::M2M100 => write!(f, "m2m100"),
            Self::Ollama => write!(f, "ollama"),
        }
    }
}

impl std::str::FromStr for TranslationEngineType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "m2m100" | "m2m-100" | "m2m" => Ok(Self::M2M100),
            "ollama" | "llm" => Ok(Self::Ollama),
            _ => Err(format!("Unknown translation engine: {}", s)),
        }
    }
}

/// Unified translator that can use either M2M-100 or Ollama backend.
///
/// This enum provides a common interface for translation regardless of the
/// underlying engine, making it easy to switch between backends.
pub enum Translator {
    /// M2M-100 neural translation engine
    M2M100(Arc<M2M100Engine>),
    /// Ollama LLM-based translation
    Ollama(Arc<OllamaTranslator>),
}

impl Translator {
    /// Translate text from source to target language.
    pub async fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        match self {
            Translator::M2M100(engine) => {
                // M2M-100 doesn't support auto detection, use detected language or default
                let actual_from = if from == "auto" {
                    warn!("M2M-100 doesn't support auto language detection, defaulting to 'en'");
                    "en"
                } else {
                    from
                };
                engine
                    .translate_sync(text, actual_from, to)
                    .map_err(TranslationError::M2M100)
            }
            Translator::Ollama(translator) => {
                <OllamaTranslator as TranslationEngine>::translate(
                    translator.as_ref(),
                    text,
                    from,
                    to,
                )
                .await
            }
        }
    }

    /// Get the name of the translation engine.
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        match self {
            Translator::M2M100(_) => "m2m100",
            Translator::Ollama(_) => "ollama",
        }
    }

    /// Check if a language pair is supported.
    #[allow(dead_code)]
    pub fn supports_pair(&self, from: &str, to: &str) -> bool {
        match self {
            Translator::M2M100(engine) => engine.supports_pair(from, to),
            Translator::Ollama(_) => true, // Ollama supports any pair
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m2m100_language_check() {
        assert!(is_m2m100_language("en"));
        assert!(is_m2m100_language("de"));
        assert!(is_m2m100_language("zh"));
        assert!(is_m2m100_language("EN")); // case insensitive
        assert!(!is_m2m100_language("xyz"));
    }

    #[test]
    fn test_translation_engine_type_from_str() {
        assert_eq!(
            "m2m100".parse::<TranslationEngineType>().unwrap(),
            TranslationEngineType::M2M100
        );
        assert_eq!(
            "ollama".parse::<TranslationEngineType>().unwrap(),
            TranslationEngineType::Ollama
        );
        assert!("invalid".parse::<TranslationEngineType>().is_err());
    }

    #[test]
    fn test_translation_engine_type_display() {
        assert_eq!(TranslationEngineType::M2M100.to_string(), "m2m100");
        assert_eq!(TranslationEngineType::Ollama.to_string(), "ollama");
    }

    #[test]
    fn test_translation_error_display() {
        let err = TranslationError::UnsupportedLanguagePair {
            from: "xx".to_string(),
            to: "yy".to_string(),
        };
        assert!(err.to_string().contains("xx"));
        assert!(err.to_string().contains("yy"));
    }
}
