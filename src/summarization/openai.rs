//! OpenAI-compatible API provider.
//!
//! Supports OpenAI, Claude (via OpenAI-compatible endpoint), Groq, and other
//! services that implement the OpenAI Chat Completions API.

use super::provider::{LlmProvider, Message, ProviderError, ProviderResponse, Role};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// OpenAI chat request format.
#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
}

/// OpenAI message format.
#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

impl From<&Message> for OpenAiMessage {
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

/// OpenAI chat response format.
#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    model: String,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    total_tokens: u32,
}

/// Configuration for OpenAI-compatible provider.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    /// API key (resolved, not the keyring: reference)
    pub api_key: String,
    /// Model name (e.g., "gpt-4o-mini", "claude-3-haiku-20240307")
    pub model: String,
    /// Base URL (e.g., "https://api.openai.com/v1")
    pub base_url: String,
    /// Request timeout in seconds
    pub timeout_secs: u32,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 120,
        }
    }
}

/// OpenAI-compatible LLM provider.
pub struct OpenAiProvider {
    client: Client,
    config: OpenAiConfig,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider with the given configuration.
    ///
    /// The API key should already be resolved (not a keyring: reference).
    pub fn new(config: OpenAiConfig) -> Result<Self, ProviderError> {
        if config.api_key.is_empty() {
            return Err(ProviderError::AuthError("API key is required".to_string()));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .map_err(|e| ProviderError::ConfigError(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Create provider with API key from SecretStore.
    ///
    /// Resolves `keyring:` prefix if present.
    #[allow(dead_code)] // API for library users
    pub fn with_secret_store(
        mut config: OpenAiConfig,
        store: &crate::secrets::SecretStore,
    ) -> Result<Self, ProviderError> {
        // Resolve the API key if it's a keyring reference
        config.api_key = crate::secrets::resolve_secret(&config.api_key, store)
            .map_err(|e| ProviderError::AuthError(e.to_string()))?;

        Self::new(config)
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn is_available(&self) -> bool {
        // Try to list models - simple auth check
        let url = format!("{}/models", self.config.base_url);
        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn chat(&self, messages: &[Message]) -> Result<ProviderResponse, ProviderError> {
        let openai_messages: Vec<OpenAiMessage> = messages.iter().map(|m| m.into()).collect();

        let request = OpenAiChatRequest {
            model: self.config.model.clone(),
            messages: openai_messages,
        };

        let url = format!("{}/chat/completions", self.config.base_url);
        debug!("Sending chat request to OpenAI-compatible API: {}", url);

        let start = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // Check for common error types
            if status.as_u16() == 401 {
                return Err(ProviderError::AuthError(format!(
                    "Authentication failed: {}",
                    body
                )));
            }

            return Err(ProviderError::ApiError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: OpenAiChatResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let elapsed = start.elapsed();
        let tokens = result.usage.map(|u| u.total_tokens);

        info!(
            "OpenAI chat completed in {}ms (model: {}, tokens: {:?})",
            elapsed.as_millis(),
            result.model,
            tokens
        );

        let content = result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        Ok(ProviderResponse {
            content: content.trim().to_string(),
            model: result.model,
            tokens_used: tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_config_default() {
        let config = OpenAiConfig::default();
        assert!(config.api_key.is_empty());
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_openai_provider_requires_api_key() {
        let config = OpenAiConfig::default();
        let result = OpenAiProvider::new(config);
        assert!(result.is_err());
        match result {
            Err(ProviderError::AuthError(msg)) => {
                assert!(msg.contains("API key"));
            }
            _ => panic!("Expected AuthError"),
        }
    }

    #[test]
    fn test_openai_provider_new() {
        let config = OpenAiConfig {
            api_key: "sk-test-key".to_string(),
            model: "gpt-4o".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 60,
        };
        let provider = OpenAiProvider::new(config).unwrap();
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model(), "gpt-4o");
    }

    #[test]
    fn test_message_to_openai_format() {
        let system = Message::system("You are helpful");
        let user = Message::user("Hello");
        let assistant = Message::assistant("Hi!");

        let openai_system: OpenAiMessage = (&system).into();
        let openai_user: OpenAiMessage = (&user).into();
        let openai_assistant: OpenAiMessage = (&assistant).into();

        assert_eq!(openai_system.role, "system");
        assert_eq!(openai_system.content, "You are helpful");

        assert_eq!(openai_user.role, "user");
        assert_eq!(openai_user.content, "Hello");

        assert_eq!(openai_assistant.role, "assistant");
        assert_eq!(openai_assistant.content, "Hi!");
    }

    #[test]
    fn test_openai_provider_custom_base_url() {
        let config = OpenAiConfig {
            api_key: "test-key".to_string(),
            model: "llama-3.1-70b-versatile".to_string(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            timeout_secs: 30,
        };
        let provider = OpenAiProvider::new(config).unwrap();
        assert_eq!(provider.model(), "llama-3.1-70b-versatile");
    }
}
