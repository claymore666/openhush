//! LLM-based text correction using Ollama.
//!
//! Provides grammar correction and filler word removal using a local
//! Ollama instance.

use crate::config::{CorrectionConfig, FillerRemovalMode};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info};

/// Correction-related errors.
#[derive(Error, Debug)]
pub enum CorrectionError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Ollama returned error: {0}")]
    OllamaError(String),
}

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
    /// Part of Ollama API response, required for deserialization
    #[allow(dead_code)]
    done: bool,
}

/// Text corrector using Ollama.
pub struct TextCorrector {
    client: Client,
    config: CorrectionConfig,
}

impl TextCorrector {
    /// Create a new text corrector.
    pub fn new(config: CorrectionConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .unwrap_or_default();

        Self { client, config }
    }

    /// Correct text using Ollama.
    ///
    /// Returns the corrected text or the original if correction fails.
    pub async fn correct(&self, text: &str) -> Result<String, CorrectionError> {
        if text.is_empty() {
            return Ok(text.to_string());
        }

        let prompt = self.build_prompt(text);
        debug!("Sending to Ollama: {}", prompt);

        let request = OllamaRequest {
            model: self.config.ollama_model.clone(),
            prompt,
            stream: false,
        };

        let url = format!("{}/api/generate", self.config.ollama_url);

        let start = std::time::Instant::now();
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CorrectionError::OllamaError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: OllamaResponse = response.json().await?;
        let elapsed = start.elapsed();

        info!(
            "Ollama correction took {}ms ({} chars -> {} chars)",
            elapsed.as_millis(),
            text.len(),
            result.response.len()
        );

        // Clean up the response (remove quotes, trim whitespace)
        let corrected = result
            .response
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .to_string();

        debug!("Corrected: '{}' -> '{}'", text, corrected);

        Ok(corrected)
    }

    /// Build the prompt for Ollama based on configuration.
    fn build_prompt(&self, text: &str) -> String {
        let mut instructions = Vec::new();

        // Grammar and punctuation correction
        instructions.push("Fix grammar and punctuation errors.");

        // Filler word removal based on mode
        if self.config.remove_fillers {
            let filler_instruction = match self.config.filler_mode {
                FillerRemovalMode::Conservative => {
                    "Remove basic filler words: um, uh, er, hmm."
                }
                FillerRemovalMode::Moderate => {
                    "Remove filler words: um, uh, er, hmm, like (when used as filler, not as in 'I like'), you know, basically, I mean."
                }
                FillerRemovalMode::Aggressive => {
                    "Remove all filler words and hesitation markers: um, uh, er, hmm, like (as filler), you know, basically, I mean, so (at start), well (at start), right, actually, literally, honestly, I guess."
                }
            };
            instructions.push(filler_instruction);
        }

        // Important constraints
        instructions.push("Preserve the original meaning and tone.");
        instructions.push("Do not add new content.");
        instructions.push("Return only the corrected text, nothing else.");

        let system_prompt = instructions.join(" ");

        format!(
            "You are a transcription post-processor. {}\n\nInput: {}\n\nOutput:",
            system_prompt, text
        )
    }

    /// Check if Ollama is available.
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.config.ollama_url);
        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CorrectionConfig {
        CorrectionConfig {
            enabled: true,
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llama3.2:3b".to_string(),
            remove_fillers: false,
            filler_mode: FillerRemovalMode::Conservative,
            timeout_secs: 30,
        }
    }

    #[test]
    fn test_build_prompt_basic() {
        let config = test_config();
        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("hello world");

        assert!(prompt.contains("Fix grammar and punctuation"));
        assert!(prompt.contains("hello world"));
        assert!(!prompt.contains("filler"));
    }

    #[test]
    fn test_build_prompt_with_fillers_conservative() {
        let mut config = test_config();
        config.remove_fillers = true;
        config.filler_mode = FillerRemovalMode::Conservative;

        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("um hello");

        assert!(prompt.contains("um, uh, er"));
    }

    #[test]
    fn test_build_prompt_with_fillers_moderate() {
        let mut config = test_config();
        config.remove_fillers = true;
        config.filler_mode = FillerRemovalMode::Moderate;

        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("like hello");

        assert!(prompt.contains("you know, basically"));
    }

    #[test]
    fn test_build_prompt_with_fillers_aggressive() {
        let mut config = test_config();
        config.remove_fillers = true;
        config.filler_mode = FillerRemovalMode::Aggressive;

        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("so actually hello");

        assert!(prompt.contains("actually, literally"));
    }
}
