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

    // ===================
    // TextCorrector Creation Tests
    // ===================

    #[test]
    fn test_text_corrector_new() {
        let config = test_config();
        let _corrector = TextCorrector::new(config);
        // Just verify creation doesn't panic
    }

    #[test]
    fn test_text_corrector_custom_timeout() {
        let mut config = test_config();
        config.timeout_secs = 60;
        let _corrector = TextCorrector::new(config);
    }

    // ===================
    // Prompt Building Tests
    // ===================

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
    fn test_build_prompt_structure() {
        let config = test_config();
        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("test input");

        // Verify prompt structure
        assert!(prompt.contains("transcription post-processor"));
        assert!(prompt.contains("Input: test input"));
        assert!(prompt.contains("Output:"));
        assert!(prompt.contains("Preserve the original meaning"));
        assert!(prompt.contains("Do not add new content"));
    }

    #[test]
    fn test_build_prompt_with_fillers_conservative() {
        let mut config = test_config();
        config.remove_fillers = true;
        config.filler_mode = FillerRemovalMode::Conservative;

        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("um hello");

        assert!(prompt.contains("um, uh, er"));
        assert!(!prompt.contains("literally"));
    }

    #[test]
    fn test_build_prompt_with_fillers_moderate() {
        let mut config = test_config();
        config.remove_fillers = true;
        config.filler_mode = FillerRemovalMode::Moderate;

        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("like hello");

        assert!(prompt.contains("you know, basically"));
        assert!(prompt.contains("I mean"));
    }

    #[test]
    fn test_build_prompt_with_fillers_aggressive() {
        let mut config = test_config();
        config.remove_fillers = true;
        config.filler_mode = FillerRemovalMode::Aggressive;

        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("so actually hello");

        assert!(prompt.contains("actually, literally"));
        assert!(prompt.contains("I guess"));
    }

    #[test]
    fn test_build_prompt_special_characters() {
        let config = test_config();
        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("Hello, world! How are you?");

        assert!(prompt.contains("Hello, world! How are you?"));
    }

    #[test]
    fn test_build_prompt_multiline() {
        let config = test_config();
        let corrector = TextCorrector::new(config);
        let prompt = corrector.build_prompt("Line 1\nLine 2");

        assert!(prompt.contains("Line 1\nLine 2"));
    }

    // ===================
    // Async Tests
    // ===================

    #[tokio::test]
    async fn test_correct_empty_string() {
        let config = test_config();
        let corrector = TextCorrector::new(config);
        let result = corrector.correct("").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    // ===================
    // Error Tests
    // ===================

    #[test]
    fn test_correction_error_display() {
        let err = CorrectionError::OllamaError("connection failed".to_string());
        assert!(err.to_string().contains("Ollama"));
        assert!(err.to_string().contains("connection failed"));
    }

    #[test]
    fn test_correction_error_debug() {
        let err = CorrectionError::OllamaError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("OllamaError"));
    }
}
