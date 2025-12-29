//! Ollama-based translation using LLMs.
//!
//! Uses local Ollama instance for flexible translation between any language pairs.
//! Quality depends on the model used (llama3.2, mistral, aya, etc.)

use super::{TranslationEngine, TranslationError};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Ollama generate request.
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

/// Ollama generate response.
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
}

/// Ollama-based translator.
pub struct OllamaTranslator {
    client: Client,
    url: String,
    model: String,
    #[allow(dead_code)] // Used in constructor, will be used for config
    timeout_secs: u32,
}

impl OllamaTranslator {
    /// Create a new Ollama translator.
    pub fn new(url: &str, model: &str, timeout_secs: u32) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs as u64))
            .build()
            .unwrap_or_default();

        Self {
            client,
            url: url.to_string(),
            model: model.to_string(),
            timeout_secs,
        }
    }

    /// Build translation prompt.
    fn build_prompt(&self, text: &str, from: &str, to: &str) -> String {
        format!(
            r#"Translate the following text from {} to {}.
Only output the translation, nothing else. Do not add explanations or notes.

Text: {}

Translation:"#,
            language_name(from),
            language_name(to),
            text
        )
    }

    /// Check if Ollama is available.
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.url);
        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

impl TranslationEngine for OllamaTranslator {
    async fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        if text.is_empty() {
            return Ok(String::new());
        }

        let prompt = self.build_prompt(text, from, to);
        debug!("Ollama translation prompt: {}", prompt);

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
        };

        let url = format!("{}/api/generate", self.url);

        let start = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| TranslationError::Ollama(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::Ollama(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: OllamaResponse = response
            .json()
            .await
            .map_err(|e| TranslationError::Ollama(e.to_string()))?;

        let elapsed = start.elapsed();
        info!(
            "Ollama translation took {}ms ({} -> {}, {} chars)",
            elapsed.as_millis(),
            from,
            to,
            text.len()
        );

        // Clean up response
        let translated = result
            .response
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string();

        debug!("Translated: '{}' -> '{}'", text, translated);

        Ok(translated)
    }

    fn supports_pair(&self, _from: &str, _to: &str) -> bool {
        // Ollama can attempt any language pair (quality varies by model)
        true
    }

    fn name(&self) -> &str {
        "ollama"
    }
}

/// Get human-readable language name from ISO code.
fn language_name(code: &str) -> &str {
    match code.to_lowercase().as_str() {
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "ru" => "Russian",
        "uk" => "Ukrainian",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "tr" => "Turkish",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "cs" => "Czech",
        "sv" => "Swedish",
        "da" => "Danish",
        "fi" => "Finnish",
        "no" => "Norwegian",
        "el" => "Greek",
        "he" => "Hebrew",
        "hu" => "Hungarian",
        "ro" => "Romanian",
        "bg" => "Bulgarian",
        "sk" => "Slovak",
        "hr" => "Croatian",
        "sr" => "Serbian",
        "sl" => "Slovenian",
        "et" => "Estonian",
        "lv" => "Latvian",
        "lt" => "Lithuanian",
        _ => code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_name() {
        assert_eq!(language_name("en"), "English");
        assert_eq!(language_name("de"), "German");
        assert_eq!(language_name("DE"), "German"); // case insensitive
        assert_eq!(language_name("xyz"), "xyz"); // fallback to code
    }

    #[test]
    fn test_ollama_translator_new() {
        let translator = OllamaTranslator::new("http://localhost:11434", "llama3.2:3b", 30);
        assert_eq!(translator.name(), "ollama");
        assert!(translator.supports_pair("de", "en"));
        assert!(translator.supports_pair("zh", "fr"));
    }

    #[test]
    fn test_build_prompt() {
        let translator = OllamaTranslator::new("http://localhost:11434", "llama3.2:3b", 30);
        let prompt = translator.build_prompt("Hallo Welt", "de", "en");

        assert!(prompt.contains("German"));
        assert!(prompt.contains("English"));
        assert!(prompt.contains("Hallo Welt"));
    }
}
