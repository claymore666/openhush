//! Meeting summarization using LLM backends.
//!
//! Provides post-hoc summarization of transcripts for meeting minutes,
//! standup notes, and other structured outputs.
//!
//! # Usage
//!
//! ```ignore
//! use openhush::summarization::{Summarizer, template::TemplateContext};
//! use openhush::summarization::ollama::{OllamaProvider, OllamaConfig};
//!
//! let provider = OllamaProvider::new(OllamaConfig::default());
//! let summarizer = Summarizer::new(Box::new(provider));
//!
//! let ctx = TemplateContext::new(
//!     "Meeting transcript here...".to_string(),
//!     "2025-01-15".to_string(),
//!     "30 minutes".to_string(),
//! );
//!
//! let result = summarizer.summarize("meeting", &ctx).await?;
//! println!("{}", result.summary);
//! ```

pub mod ollama;
pub mod openai;
pub mod provider;
pub mod template;

pub use ollama::{OllamaConfig, OllamaProvider};
pub use openai::{OpenAiConfig, OpenAiProvider};
pub use provider::{LlmProvider, Message, ProviderError};
pub use template::{TemplateContext, TemplateError, TemplateRegistry};

// Re-exports for library users (not used by binary)
#[allow(unused_imports)]
pub use provider::ProviderResponse;
#[allow(unused_imports)]
pub use template::Template;

use thiserror::Error;
use tracing::info;

/// Summarization-related errors.
#[derive(Error, Debug)]
#[allow(dead_code)] // Variants for API completeness
pub enum SummarizationError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Template error: {0}")]
    Template(#[from] TemplateError),

    #[error("File error: {0}")]
    FileError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result of a summarization operation.
#[derive(Debug)]
pub struct SummarizationResult {
    /// The generated summary.
    pub summary: String,
    /// Name of the template used.
    pub template_used: String,
    /// Name of the LLM provider used.
    pub provider_used: String,
    /// Model that generated the summary.
    pub model_used: String,
    /// Tokens used (if reported by provider).
    pub tokens_used: Option<u32>,
}

/// High-level summarizer that orchestrates providers and templates.
pub struct Summarizer {
    provider: Box<dyn LlmProvider>,
    templates: TemplateRegistry,
}

impl Summarizer {
    /// Create a new summarizer with the given LLM provider.
    pub fn new(provider: Box<dyn LlmProvider>) -> Self {
        Self {
            provider,
            templates: TemplateRegistry::new(),
        }
    }

    /// Load custom templates from a file.
    #[allow(dead_code)] // API for library users
    pub fn with_custom_templates(
        mut self,
        path: &std::path::Path,
    ) -> Result<Self, SummarizationError> {
        self.templates.load_custom(path)?;
        Ok(self)
    }

    /// Get the template registry for listing available templates.
    #[allow(dead_code)] // API for library users
    pub fn templates(&self) -> &TemplateRegistry {
        &self.templates
    }

    /// Check if the LLM provider is available.
    #[allow(dead_code)] // API for library users
    pub async fn is_provider_available(&self) -> bool {
        self.provider.is_available().await
    }

    /// Summarize a transcript using the specified template.
    pub async fn summarize(
        &self,
        template_name: &str,
        ctx: &TemplateContext,
    ) -> Result<SummarizationResult, SummarizationError> {
        let template = self
            .templates
            .get(template_name)
            .ok_or_else(|| TemplateError::NotFound(template_name.to_string()))?;

        let (system_prompt, user_prompt) = template.render(ctx);

        info!(
            "Summarizing with template '{}' using {} ({})",
            template_name,
            self.provider.name(),
            self.provider.model()
        );

        let messages = vec![Message::system(system_prompt), Message::user(user_prompt)];

        let response = self.provider.chat(&messages).await?;

        Ok(SummarizationResult {
            summary: response.content,
            template_used: template_name.to_string(),
            provider_used: self.provider.name().to_string(),
            model_used: response.model,
            tokens_used: response.tokens_used,
        })
    }
}

/// List all available built-in templates.
pub fn list_templates() {
    let registry = TemplateRegistry::new();
    println!("Available templates:\n");
    for name in registry.list() {
        if let Some(template) = registry.get(name) {
            println!("  {} - {}", name, template.description);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock provider for testing
    struct MockProvider {
        response: String,
    }

    impl MockProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn model(&self) -> &str {
            "mock-model"
        }

        async fn is_available(&self) -> bool {
            true
        }

        async fn chat(&self, _messages: &[Message]) -> Result<ProviderResponse, ProviderError> {
            Ok(ProviderResponse {
                content: self.response.clone(),
                model: "mock-model".to_string(),
                tokens_used: Some(100),
            })
        }
    }

    #[tokio::test]
    async fn test_summarizer_new() {
        let provider = MockProvider::new("Test summary");
        let summarizer = Summarizer::new(Box::new(provider));
        assert!(summarizer.is_provider_available().await);
    }

    #[tokio::test]
    async fn test_summarizer_summarize() {
        let provider = MockProvider::new("# Meeting Summary\n\nThis is a test.");
        let summarizer = Summarizer::new(Box::new(provider));

        let ctx = TemplateContext::new(
            "Test transcript".to_string(),
            "2025-01-15".to_string(),
            "30 minutes".to_string(),
        );

        let result = summarizer.summarize("meeting", &ctx).await.unwrap();

        assert_eq!(result.summary, "# Meeting Summary\n\nThis is a test.");
        assert_eq!(result.template_used, "meeting");
        assert_eq!(result.provider_used, "mock");
        assert_eq!(result.model_used, "mock-model");
        assert_eq!(result.tokens_used, Some(100));
    }

    #[tokio::test]
    async fn test_summarizer_template_not_found() {
        let provider = MockProvider::new("Test");
        let summarizer = Summarizer::new(Box::new(provider));

        let ctx = TemplateContext::new("Test".to_string(), "date".to_string(), "1m".to_string());

        let result = summarizer.summarize("nonexistent", &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_summarizer_all_templates() {
        let provider = MockProvider::new("Summary");
        let summarizer = Summarizer::new(Box::new(provider));

        let ctx = TemplateContext::new(
            "Transcript".to_string(),
            "2025-01-15".to_string(),
            "1 hour".to_string(),
        );

        // Test all built-in templates work
        for template_name in summarizer.templates().list() {
            let result = summarizer.summarize(template_name, &ctx).await;
            assert!(result.is_ok(), "Template '{}' failed", template_name);
        }
    }

    #[test]
    fn test_summarization_error_display() {
        let err = SummarizationError::FileError("not found".to_string());
        assert!(err.to_string().contains("File error"));

        let err = SummarizationError::ConfigError("invalid".to_string());
        assert!(err.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_list_templates() {
        // Just verify it doesn't panic
        list_templates();
    }
}
