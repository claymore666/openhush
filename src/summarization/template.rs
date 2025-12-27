//! Summarization templates for different meeting types.
//!
//! Provides built-in templates for common meeting formats and supports
//! custom templates loaded from configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Built-in template names.
pub const TEMPLATE_STANDUP: &str = "standup";
pub const TEMPLATE_MEETING: &str = "meeting";
pub const TEMPLATE_RETRO: &str = "retro";
pub const TEMPLATE_1ON1: &str = "1on1";
pub const TEMPLATE_SUMMARY: &str = "summary";

/// Template-related errors.
#[derive(Error, Debug)]
#[allow(dead_code)] // Variants for API completeness
pub enum TemplateError {
    #[error("Template not found: {0}")]
    NotFound(String),

    #[error("Failed to load templates: {0}")]
    LoadError(String),

    #[error("Invalid template format: {0}")]
    InvalidFormat(String),
}

/// A summarization template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Template name/identifier.
    pub name: String,
    /// Description of the template.
    pub description: String,
    /// System prompt for the LLM.
    pub system_prompt: String,
    /// User prompt template (supports {transcript}, {date}, {duration}).
    pub user_prompt: String,
}

impl Template {
    /// Render the template with the given context.
    ///
    /// Returns (system_prompt, user_prompt) with variables substituted.
    pub fn render(&self, ctx: &TemplateContext) -> (String, String) {
        let user = self
            .user_prompt
            .replace("{transcript}", &ctx.transcript)
            .replace("{date}", &ctx.date)
            .replace("{duration}", &ctx.duration);

        (self.system_prompt.clone(), user)
    }
}

/// Context for template rendering.
#[derive(Debug, Clone)]
pub struct TemplateContext {
    /// The transcript text to summarize.
    pub transcript: String,
    /// Date of the recording/meeting (YYYY-MM-DD format).
    pub date: String,
    /// Duration of the recording (e.g., "30 minutes").
    pub duration: String,
}

impl TemplateContext {
    /// Create a new template context.
    pub fn new(transcript: String, date: String, duration: String) -> Self {
        Self {
            transcript,
            date,
            duration,
        }
    }
}

/// Template registry with built-in and custom templates.
pub struct TemplateRegistry {
    templates: HashMap<String, Template>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    /// Create a new registry with built-in templates.
    pub fn new() -> Self {
        let mut registry = Self {
            templates: HashMap::new(),
        };
        registry.register_builtin();
        registry
    }

    /// Get a template by name.
    pub fn get(&self, name: &str) -> Option<&Template> {
        self.templates.get(name)
    }

    /// List all available template names.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.templates.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Load custom templates from a TOML file.
    #[allow(dead_code)] // API for library users
    pub fn load_custom(&mut self, path: &std::path::Path) -> Result<(), TemplateError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TemplateError::LoadError(format!("{}: {}", path.display(), e)))?;

        let custom: HashMap<String, Template> =
            toml::from_str(&content).map_err(|e| TemplateError::InvalidFormat(e.to_string()))?;

        for (name, template) in custom {
            self.templates.insert(name, template);
        }

        Ok(())
    }

    /// Register all built-in templates.
    fn register_builtin(&mut self) {
        self.templates
            .insert(TEMPLATE_STANDUP.to_string(), Self::standup_template());
        self.templates
            .insert(TEMPLATE_MEETING.to_string(), Self::meeting_template());
        self.templates
            .insert(TEMPLATE_RETRO.to_string(), Self::retro_template());
        self.templates
            .insert(TEMPLATE_1ON1.to_string(), Self::one_on_one_template());
        self.templates
            .insert(TEMPLATE_SUMMARY.to_string(), Self::summary_template());
    }

    fn standup_template() -> Template {
        Template {
            name: TEMPLATE_STANDUP.to_string(),
            description: "Daily standup meeting summary with yesterday/today/blockers format"
                .to_string(),
            system_prompt: r#"You are a meeting summarizer specializing in daily standup meetings. Create concise, well-structured standup notes in markdown format. Focus on extracting actionable information."#.to_string(),
            user_prompt: r#"Summarize this daily standup meeting transcript. For each participant mentioned, extract:

1. **Yesterday**: What they completed
2. **Today**: What they plan to work on
3. **Blockers**: Any impediments or issues

Format the output as markdown with clear sections for each person.

---

**Date:** {date}
**Duration:** {duration}

**Transcript:**
{transcript}"#.to_string(),
        }
    }

    fn meeting_template() -> Template {
        Template {
            name: TEMPLATE_MEETING.to_string(),
            description: "General meeting minutes with discussion points, decisions, and action items".to_string(),
            system_prompt: r#"You are a professional meeting minutes writer. Create clear, comprehensive meeting notes that capture key discussions, decisions, and next steps. Use markdown formatting."#.to_string(),
            user_prompt: r#"Create meeting minutes from this transcript. Include:

1. **Attendees** (if identifiable from the conversation)
2. **Key Discussion Points** - Main topics covered
3. **Decisions Made** - Any conclusions or agreements reached
4. **Action Items** - Tasks assigned, with owners if mentioned
5. **Next Steps** - Follow-up items or future meeting topics

Format as professional markdown meeting notes.

---

**Date:** {date}
**Duration:** {duration}

**Transcript:**
{transcript}"#.to_string(),
        }
    }

    fn retro_template() -> Template {
        Template {
            name: TEMPLATE_RETRO.to_string(),
            description: "Sprint retrospective with what went well, improvements, and action items"
                .to_string(),
            system_prompt: r#"You are a retrospective facilitator creating structured notes from team retrospectives. Focus on capturing honest feedback and actionable improvements."#.to_string(),
            user_prompt: r#"Summarize this sprint retrospective meeting. Organize into:

1. **What Went Well** - Positive outcomes and successes
2. **What Could Be Improved** - Challenges and areas for improvement
3. **Action Items** - Specific improvements to try next sprint

Be concise but capture the team's sentiment accurately.

---

**Date:** {date}
**Duration:** {duration}

**Transcript:**
{transcript}"#.to_string(),
        }
    }

    fn one_on_one_template() -> Template {
        Template {
            name: TEMPLATE_1ON1.to_string(),
            description: "One-on-one meeting notes with discussion points, feedback, and goals"
                .to_string(),
            system_prompt: r#"You are summarizing a one-on-one meeting between a manager and team member. Be respectful of the personal nature of these conversations while capturing key points."#.to_string(),
            user_prompt: r#"Create notes from this one-on-one meeting. Include:

1. **Topics Discussed** - Main subjects covered
2. **Feedback Shared** - Any performance or project feedback
3. **Concerns Raised** - Issues or challenges mentioned
4. **Goals & Development** - Career growth, learning, or project goals
5. **Action Items** - Follow-up tasks for either party

Keep the tone professional but warm.

---

**Date:** {date}
**Duration:** {duration}

**Transcript:**
{transcript}"#.to_string(),
        }
    }

    fn summary_template() -> Template {
        Template {
            name: TEMPLATE_SUMMARY.to_string(),
            description: "Simple transcript summary without structured format".to_string(),
            system_prompt: r#"You are a concise summarizer. Create a clear, readable summary of the transcript that captures the main points without unnecessary detail."#.to_string(),
            user_prompt: r#"Provide a concise summary of this transcript. Capture:

- The main topic or purpose
- Key points discussed
- Any conclusions or outcomes

Keep it brief but informative.

---

**Date:** {date}
**Duration:** {duration}

**Transcript:**
{transcript}"#.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_registry_new() {
        let registry = TemplateRegistry::new();
        assert!(registry.get(TEMPLATE_STANDUP).is_some());
        assert!(registry.get(TEMPLATE_MEETING).is_some());
        assert!(registry.get(TEMPLATE_RETRO).is_some());
        assert!(registry.get(TEMPLATE_1ON1).is_some());
        assert!(registry.get(TEMPLATE_SUMMARY).is_some());
    }

    #[test]
    fn test_template_registry_list() {
        let registry = TemplateRegistry::new();
        let templates = registry.list();
        assert_eq!(templates.len(), 5);
        assert!(templates.contains(&"standup"));
        assert!(templates.contains(&"meeting"));
    }

    #[test]
    fn test_template_render() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TEMPLATE_SUMMARY).unwrap();

        let ctx = TemplateContext::new(
            "Hello world transcript".to_string(),
            "2025-01-15".to_string(),
            "30 minutes".to_string(),
        );

        let (system, user) = template.render(&ctx);

        assert!(system.contains("summarizer"));
        assert!(user.contains("Hello world transcript"));
        assert!(user.contains("2025-01-15"));
        assert!(user.contains("30 minutes"));
    }

    #[test]
    fn test_template_render_substitution() {
        let template = Template {
            name: "test".to_string(),
            description: "test".to_string(),
            system_prompt: "System".to_string(),
            user_prompt: "Date: {date}, Duration: {duration}, Text: {transcript}".to_string(),
        };

        let ctx = TemplateContext::new(
            "My text".to_string(),
            "2025-01-01".to_string(),
            "5m".to_string(),
        );

        let (_, user) = template.render(&ctx);
        assert_eq!(user, "Date: 2025-01-01, Duration: 5m, Text: My text");
    }

    #[test]
    fn test_template_context_new() {
        let ctx = TemplateContext::new(
            "transcript".to_string(),
            "date".to_string(),
            "duration".to_string(),
        );
        assert_eq!(ctx.transcript, "transcript");
        assert_eq!(ctx.date, "date");
        assert_eq!(ctx.duration, "duration");
    }

    #[test]
    fn test_template_not_found() {
        let registry = TemplateRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_builtin_templates_have_required_fields() {
        let registry = TemplateRegistry::new();
        for name in registry.list() {
            let template = registry.get(name).unwrap();
            assert!(!template.name.is_empty());
            assert!(!template.description.is_empty());
            assert!(!template.system_prompt.is_empty());
            assert!(template.user_prompt.contains("{transcript}"));
        }
    }
}
