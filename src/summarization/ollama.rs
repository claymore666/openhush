//! Ollama LLM provider for local inference.
//!
//! Provides access to locally running Ollama models for summarization.

use super::provider::{LlmProvider, Message, ProviderError, ProviderResponse, Role};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Ollama chat request.
#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
}

/// Ollama chat message format.
#[derive(Debug, Serialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

impl From<&Message> for OllamaChatMessage {
    fn from(msg: &Message) -> Self {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        Self {
            role: role.to_string(),
            content: msg.content.clone(),
        }
    }
}

/// Ollama chat response.
#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    #[allow(dead_code)]
    done: bool,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

/// Configuration for Ollama provider.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Ollama API URL (e.g., "http://localhost:11434")
    pub url: String,
    /// Model name (e.g., "llama3.2:3b")
    pub model: String,
    /// Request timeout in seconds
    pub timeout_secs: u32,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:11434".to_string(),
            model: "llama3.2:3b".to_string(),
            timeout_secs: 120,
        }
    }
}

/// Ollama LLM provider.
pub struct OllamaProvider {
    client: Client,
    config: OllamaConfig,
}

impl OllamaProvider {
    /// Create a new Ollama provider with the given configuration.
    pub fn new(config: OllamaConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .unwrap_or_default();

        Self { client, config }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.config.url);
        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn chat(&self, messages: &[Message]) -> Result<ProviderResponse, ProviderError> {
        let ollama_messages: Vec<OllamaChatMessage> = messages.iter().map(|m| m.into()).collect();

        let request = OllamaChatRequest {
            model: self.config.model.clone(),
            messages: ollama_messages,
            stream: false,
        };

        let url = format!("{}/api/chat", self.config.url);
        debug!("Sending chat request to Ollama: {}", url);

        let start = std::time::Instant::now();
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let elapsed = start.elapsed();
        let tokens = result
            .eval_count
            .zip(result.prompt_eval_count)
            .map(|(e, p)| e + p);

        info!(
            "Ollama chat completed in {}ms (model: {}, tokens: {:?})",
            elapsed.as_millis(),
            self.config.model,
            tokens
        );

        Ok(ProviderResponse {
            content: result.message.content.trim().to_string(),
            model: self.config.model.clone(),
            tokens_used: tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config_default() {
        let config = OllamaConfig::default();
        assert_eq!(config.url, "http://localhost:11434");
        assert_eq!(config.model, "llama3.2:3b");
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_ollama_provider_new() {
        let config = OllamaConfig::default();
        let provider = OllamaProvider::new(config);
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model(), "llama3.2:3b");
    }

    #[test]
    fn test_message_to_ollama_format() {
        let system = Message::system("You are helpful");
        let user = Message::user("Hello");
        let assistant = Message::assistant("Hi!");

        let ollama_system: OllamaChatMessage = (&system).into();
        let ollama_user: OllamaChatMessage = (&user).into();
        let ollama_assistant: OllamaChatMessage = (&assistant).into();

        assert_eq!(ollama_system.role, "system");
        assert_eq!(ollama_system.content, "You are helpful");

        assert_eq!(ollama_user.role, "user");
        assert_eq!(ollama_user.content, "Hello");

        assert_eq!(ollama_assistant.role, "assistant");
        assert_eq!(ollama_assistant.content, "Hi!");
    }

    #[test]
    fn test_ollama_provider_custom_config() {
        let config = OllamaConfig {
            url: "http://192.168.1.100:11434".to_string(),
            model: "mistral:7b".to_string(),
            timeout_secs: 300,
        };
        let provider = OllamaProvider::new(config);
        assert_eq!(provider.model(), "mistral:7b");
    }
}
