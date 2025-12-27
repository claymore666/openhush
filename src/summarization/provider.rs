//! LLM provider abstraction for summarization.
//!
//! Defines the `LlmProvider` trait and common types for interacting with
//! different LLM backends (Ollama, OpenAI-compatible APIs).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Errors from LLM providers.
#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Provider returned error: {0}")]
    ApiError(String),

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Request timed out after {0:?}")]
    #[allow(dead_code)] // API for library users
    Timeout(Duration),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Message role for chat-style prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Create an assistant message.
    #[allow(dead_code)] // API for library users
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Response from an LLM provider.
#[derive(Debug)]
pub struct ProviderResponse {
    /// The generated content.
    pub content: String,
    /// The model used for generation.
    pub model: String,
    /// Number of tokens used (if available).
    pub tokens_used: Option<u32>,
}

/// Trait for LLM providers.
///
/// Implementations provide access to different LLM backends.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name (e.g., "ollama", "openai").
    fn name(&self) -> &'static str;

    /// Get the model name being used.
    fn model(&self) -> &str;

    /// Check if the provider is available/reachable.
    async fn is_available(&self) -> bool;

    /// Send a chat completion request.
    ///
    /// This is the primary method for generating text with system/user roles.
    async fn chat(&self, messages: &[Message]) -> Result<ProviderResponse, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_system() {
        let msg = Message::system("You are helpful");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "You are helpful");
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("Hi there!");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hi there!");
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::AuthError("invalid key".to_string());
        assert!(err.to_string().contains("Authentication failed"));
        assert!(err.to_string().contains("invalid key"));
    }

    #[test]
    fn test_provider_error_timeout() {
        let err = ProviderError::Timeout(Duration::from_secs(30));
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }
}
