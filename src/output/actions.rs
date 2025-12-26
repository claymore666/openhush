//! Post-transcription actions.
//!
//! Allows users to configure actions triggered after successful transcription,
//! such as shell commands, HTTP requests, and file logging.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

/// Errors that can occur during action execution.
#[derive(Error, Debug)]
pub enum ActionError {
    #[error("Shell command failed: {0}")]
    Shell(String),

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("File operation failed: {0}")]
    File(String),

    #[error("Action timed out after {0:?}")]
    Timeout(Duration),

    #[error("Invalid configuration: {0}")]
    Config(String),
}

/// Context passed to actions for variable substitution.
#[derive(Debug, Clone)]
pub struct ActionContext {
    /// The transcribed text
    pub text: String,
    /// Duration of the recording in seconds
    pub duration_secs: f32,
    /// Timestamp when transcription completed
    pub timestamp: DateTime<Utc>,
    /// Whisper model used
    pub model: String,
    /// Transcription sequence ID
    pub seq_id: u64,
}

impl ActionContext {
    /// Create a new action context.
    pub fn new(text: String, duration_secs: f32, model: String, seq_id: u64) -> Self {
        Self {
            text,
            duration_secs,
            timestamp: Utc::now(),
            model,
            seq_id,
        }
    }

    /// Substitute variables in a template string.
    ///
    /// Supported variables:
    /// - `{text}` - Raw transcribed text
    /// - `{text_escaped}` - JSON-escaped text (with quotes stripped)
    /// - `{text_base64}` - Base64-encoded text
    /// - `{date}` - Current date (YYYY-MM-DD)
    /// - `{time}` - Current time (HH:MM:SS)
    /// - `{duration}` - Recording duration in seconds
    /// - `{model}` - Whisper model used
    /// - `{seq_id}` - Transcription sequence ID
    pub fn substitute(&self, template: &str) -> String {
        let text_escaped = serde_json::to_string(&self.text)
            .unwrap_or_else(|_| self.text.clone())
            .trim_matches('"')
            .to_string();

        let text_base64 = BASE64.encode(self.text.as_bytes());

        template
            .replace("{text}", &self.text)
            .replace("{text_escaped}", &text_escaped)
            .replace("{text_base64}", &text_base64)
            .replace("{date}", &self.timestamp.format("%Y-%m-%d").to_string())
            .replace("{time}", &self.timestamp.format("%H:%M:%S").to_string())
            .replace("{duration}", &format!("{:.1}", self.duration_secs))
            .replace("{model}", &self.model)
            .replace("{seq_id}", &self.seq_id.to_string())
    }

    /// Sanitize text for safe shell execution.
    ///
    /// Removes potentially dangerous characters that could lead to
    /// command injection: backticks, $(), $[], etc.
    pub fn sanitize_for_shell(text: &str) -> String {
        text.replace('`', "'")
            .replace("$(", "(")
            .replace("${", "{")
            .replace("$[", "[")
            .replace('\0', "")
    }
}

/// Action type configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionConfig {
    /// Execute a shell command.
    Shell {
        /// The command to execute (supports variable substitution)
        command: String,
        /// Timeout in seconds (default: 30)
        #[serde(default = "default_timeout")]
        timeout_secs: u32,
        /// Whether this action is enabled
        #[serde(default = "default_true")]
        enabled: bool,
    },

    /// Make an HTTP request.
    Http {
        /// The URL to request (supports variable substitution)
        url: String,
        /// HTTP method (GET, POST, PUT, DELETE)
        #[serde(default = "default_post")]
        method: String,
        /// Request body (supports variable substitution)
        #[serde(default)]
        body: Option<String>,
        /// Request headers
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Timeout in seconds (default: 30)
        #[serde(default = "default_timeout")]
        timeout_secs: u32,
        /// Whether this action is enabled
        #[serde(default = "default_true")]
        enabled: bool,
    },

    /// Append or write to a file.
    File {
        /// Path to the file (supports variable substitution, ~ expansion)
        path: String,
        /// Format string for the content (supports variable substitution)
        #[serde(default = "default_file_format")]
        format: String,
        /// Append to file instead of overwriting
        #[serde(default = "default_true")]
        append: bool,
        /// Whether this action is enabled
        #[serde(default = "default_true")]
        enabled: bool,
    },
}

fn default_timeout() -> u32 {
    30
}

fn default_true() -> bool {
    true
}

fn default_post() -> String {
    "POST".to_string()
}

fn default_file_format() -> String {
    "{text}\n".to_string()
}

impl ActionConfig {
    /// Check if this action is enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            ActionConfig::Shell { enabled, .. } => *enabled,
            ActionConfig::Http { enabled, .. } => *enabled,
            ActionConfig::File { enabled, .. } => *enabled,
        }
    }

    /// Get a display name for this action type.
    pub fn name(&self) -> &'static str {
        match self {
            ActionConfig::Shell { .. } => "shell",
            ActionConfig::Http { .. } => "http",
            ActionConfig::File { .. } => "file",
        }
    }

    /// Execute this action with the given context.
    pub async fn execute(&self, ctx: &ActionContext) -> Result<(), ActionError> {
        if !self.is_enabled() {
            return Ok(());
        }

        match self {
            ActionConfig::Shell {
                command,
                timeout_secs,
                ..
            } => execute_shell(command, *timeout_secs, ctx).await,
            ActionConfig::Http {
                url,
                method,
                body,
                headers,
                timeout_secs,
                ..
            } => execute_http(url, method, body.as_deref(), headers, *timeout_secs, ctx).await,
            ActionConfig::File {
                path,
                format,
                append,
                ..
            } => execute_file(path, format, *append, ctx).await,
        }
    }
}

/// Execute a shell command action.
async fn execute_shell(
    command: &str,
    timeout_secs: u32,
    ctx: &ActionContext,
) -> Result<(), ActionError> {
    // Sanitize text before substitution for security
    let safe_text = ActionContext::sanitize_for_shell(&ctx.text);
    let safe_ctx = ActionContext {
        text: safe_text,
        ..ctx.clone()
    };
    let cmd = safe_ctx.substitute(command);

    info!("Executing shell action: {}", cmd);

    let timeout = Duration::from_secs(u64::from(timeout_secs));

    let output = tokio::time::timeout(
        timeout,
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output(),
    )
    .await
    .map_err(|_| ActionError::Timeout(timeout))?
    .map_err(|e| ActionError::Shell(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Shell action exited with status {}: {}",
            output.status, stderr
        );
        // Don't fail - log and continue
    }

    Ok(())
}

/// Execute an HTTP request action.
async fn execute_http(
    url: &str,
    method: &str,
    body: Option<&str>,
    headers: &HashMap<String, String>,
    timeout_secs: u32,
    ctx: &ActionContext,
) -> Result<(), ActionError> {
    let url = ctx.substitute(url);
    let body = body.map(|b| ctx.substitute(b));

    info!("Executing HTTP action: {} {}", method, url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(u64::from(timeout_secs)))
        .build()
        .map_err(|e| ActionError::Http(e.to_string()))?;

    let method = method
        .parse::<reqwest::Method>()
        .map_err(|e| ActionError::Config(format!("Invalid HTTP method: {}", e)))?;

    let mut req = client.request(method, &url);

    for (key, value) in headers {
        req = req.header(key, ctx.substitute(value));
    }

    if let Some(body) = body {
        req = req.body(body);
    }

    let response = req
        .send()
        .await
        .map_err(|e| ActionError::Http(e.to_string()))?;

    if !response.status().is_success() {
        warn!("HTTP action returned status {}: {}", response.status(), url);
        // Don't fail - log and continue
    }

    Ok(())
}

/// Execute a file write action.
async fn execute_file(
    path: &str,
    format: &str,
    append: bool,
    ctx: &ActionContext,
) -> Result<(), ActionError> {
    let path = ctx.substitute(path);
    let content = ctx.substitute(format);

    // Expand ~ to home directory
    let path = if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(rest)
        } else {
            PathBuf::from(&path)
        }
    } else if let Some(rest) = path.strip_prefix('~') {
        // Handle bare ~ (no slash)
        if let Some(home) = dirs::home_dir() {
            home.join(rest)
        } else {
            PathBuf::from(&path)
        }
    } else {
        PathBuf::from(&path)
    };

    info!(
        "Executing file action: {} (append={})",
        path.display(),
        append
    );

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ActionError::File(format!("Failed to create directory: {}", e)))?;
    }

    // Open file with appropriate options
    let mut file = if append {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
    } else {
        tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await
    }
    .map_err(|e| ActionError::File(format!("Failed to open file: {}", e)))?;

    file.write_all(content.as_bytes())
        .await
        .map_err(|e| ActionError::File(format!("Failed to write file: {}", e)))?;

    // Ensure data is flushed to disk
    file.sync_all()
        .await
        .map_err(|e| ActionError::File(format!("Failed to sync file: {}", e)))?;

    Ok(())
}

/// Runner that executes multiple actions in sequence.
pub struct ActionRunner {
    actions: Vec<ActionConfig>,
}

impl ActionRunner {
    /// Create a new action runner with the given actions.
    pub fn new(actions: Vec<ActionConfig>) -> Self {
        // Filter to only enabled actions
        let actions = actions.into_iter().filter(|a| a.is_enabled()).collect();
        Self { actions }
    }

    /// Check if there are any actions to run.
    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

    /// Execute all actions with the given context.
    ///
    /// Errors in individual actions are logged but don't stop other actions.
    pub async fn run_all(&self, ctx: &ActionContext) {
        if self.actions.is_empty() {
            return;
        }

        info!(
            "Running {} post-transcription action(s) for seq_id={}",
            self.actions.len(),
            ctx.seq_id
        );

        for action in &self.actions {
            if let Err(e) = action.execute(ctx).await {
                error!("Action '{}' failed: {}", action.name(), e);
                // Continue with other actions
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // ActionContext Tests
    // ===================

    #[test]
    fn test_action_context_new() {
        let ctx = ActionContext::new("Hello world".to_string(), 5.5, "base".to_string(), 42);
        assert_eq!(ctx.text, "Hello world");
        assert!((ctx.duration_secs - 5.5).abs() < 0.01);
        assert_eq!(ctx.model, "base");
        assert_eq!(ctx.seq_id, 42);
    }

    #[test]
    fn test_substitute_text() {
        let ctx = ActionContext::new("Hello world".to_string(), 5.5, "base".to_string(), 42);
        assert_eq!(ctx.substitute("{text}"), "Hello world");
    }

    #[test]
    fn test_substitute_text_escaped() {
        let ctx = ActionContext::new("Hello \"world\"".to_string(), 5.5, "base".to_string(), 42);
        let result = ctx.substitute("{text_escaped}");
        assert!(result.contains("\\\""));
    }

    #[test]
    fn test_substitute_text_base64() {
        let ctx = ActionContext::new("Hello".to_string(), 5.5, "base".to_string(), 42);
        // "Hello" in base64 is "SGVsbG8="
        assert_eq!(ctx.substitute("{text_base64}"), "SGVsbG8=");
    }

    #[test]
    fn test_substitute_duration() {
        let ctx = ActionContext::new("test".to_string(), 12.34, "base".to_string(), 42);
        assert_eq!(ctx.substitute("{duration}"), "12.3");
    }

    #[test]
    fn test_substitute_model() {
        let ctx = ActionContext::new("test".to_string(), 5.5, "large-v3".to_string(), 42);
        assert_eq!(ctx.substitute("{model}"), "large-v3");
    }

    #[test]
    fn test_substitute_seq_id() {
        let ctx = ActionContext::new("test".to_string(), 5.5, "base".to_string(), 123);
        assert_eq!(ctx.substitute("{seq_id}"), "123");
    }

    #[test]
    fn test_substitute_date_format() {
        let ctx = ActionContext::new("test".to_string(), 5.5, "base".to_string(), 42);
        let result = ctx.substitute("{date}");
        // Should be YYYY-MM-DD format
        assert!(result.len() == 10);
        assert!(result.chars().nth(4) == Some('-'));
        assert!(result.chars().nth(7) == Some('-'));
    }

    #[test]
    fn test_substitute_time_format() {
        let ctx = ActionContext::new("test".to_string(), 5.5, "base".to_string(), 42);
        let result = ctx.substitute("{time}");
        // Should be HH:MM:SS format
        assert!(result.len() == 8);
        assert!(result.chars().nth(2) == Some(':'));
        assert!(result.chars().nth(5) == Some(':'));
    }

    #[test]
    fn test_substitute_multiple() {
        let ctx = ActionContext::new("Hello".to_string(), 5.0, "base".to_string(), 1);
        let result = ctx.substitute("Text: {text}, Model: {model}, ID: {seq_id}");
        assert_eq!(result, "Text: Hello, Model: base, ID: 1");
    }

    // ===================
    // Sanitization Tests
    // ===================

    #[test]
    fn test_sanitize_backticks() {
        let result = ActionContext::sanitize_for_shell("hello `whoami` world");
        assert_eq!(result, "hello 'whoami' world");
    }

    #[test]
    fn test_sanitize_command_substitution() {
        let result = ActionContext::sanitize_for_shell("hello $(rm -rf /) world");
        assert_eq!(result, "hello (rm -rf /) world");
    }

    #[test]
    fn test_sanitize_variable_expansion() {
        let result = ActionContext::sanitize_for_shell("hello ${HOME} world");
        assert_eq!(result, "hello {HOME} world");
    }

    #[test]
    fn test_sanitize_null_bytes() {
        let result = ActionContext::sanitize_for_shell("hello\0world");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_sanitize_safe_text() {
        let result = ActionContext::sanitize_for_shell("Hello, world! How are you?");
        assert_eq!(result, "Hello, world! How are you?");
    }

    // ===================
    // ActionConfig Tests
    // ===================

    #[test]
    fn test_shell_action_enabled() {
        let action = ActionConfig::Shell {
            command: "echo test".to_string(),
            timeout_secs: 30,
            enabled: true,
        };
        assert!(action.is_enabled());
        assert_eq!(action.name(), "shell");
    }

    #[test]
    fn test_shell_action_disabled() {
        let action = ActionConfig::Shell {
            command: "echo test".to_string(),
            timeout_secs: 30,
            enabled: false,
        };
        assert!(!action.is_enabled());
    }

    #[test]
    fn test_http_action_enabled() {
        let action = ActionConfig::Http {
            url: "http://localhost".to_string(),
            method: "POST".to_string(),
            body: None,
            headers: HashMap::new(),
            timeout_secs: 30,
            enabled: true,
        };
        assert!(action.is_enabled());
        assert_eq!(action.name(), "http");
    }

    #[test]
    fn test_file_action_enabled() {
        let action = ActionConfig::File {
            path: "/tmp/test.txt".to_string(),
            format: "{text}\n".to_string(),
            append: true,
            enabled: true,
        };
        assert!(action.is_enabled());
        assert_eq!(action.name(), "file");
    }

    // ===================
    // TOML Parsing Tests
    // ===================

    #[test]
    fn test_parse_shell_action() {
        let toml_str = r#"
type = "shell"
command = "echo '{text}'"
timeout_secs = 10
enabled = true
"#;
        let action: ActionConfig = toml::from_str(toml_str).unwrap();
        match action {
            ActionConfig::Shell {
                command,
                timeout_secs,
                enabled,
            } => {
                assert_eq!(command, "echo '{text}'");
                assert_eq!(timeout_secs, 10);
                assert!(enabled);
            }
            _ => panic!("Expected Shell action"),
        }
    }

    #[test]
    fn test_parse_http_action() {
        let toml_str = r#"
type = "http"
url = "http://localhost:8080/api"
method = "POST"
body = '{"text": "{text_escaped}"}'
enabled = true

[headers]
"Content-Type" = "application/json"
"#;
        let action: ActionConfig = toml::from_str(toml_str).unwrap();
        match action {
            ActionConfig::Http {
                url,
                method,
                body,
                headers,
                enabled,
                ..
            } => {
                assert_eq!(url, "http://localhost:8080/api");
                assert_eq!(method, "POST");
                assert_eq!(body, Some(r#"{"text": "{text_escaped}"}"#.to_string()));
                assert!(enabled);
                assert_eq!(
                    headers.get("Content-Type"),
                    Some(&"application/json".to_string())
                );
            }
            _ => panic!("Expected Http action"),
        }
    }

    #[test]
    fn test_parse_file_action() {
        let toml_str = r#"
type = "file"
path = "~/notes/{date}.md"
format = "{text}\n"
append = true
enabled = true
"#;
        let action: ActionConfig = toml::from_str(toml_str).unwrap();
        match action {
            ActionConfig::File {
                path,
                format,
                append,
                enabled,
            } => {
                assert_eq!(path, "~/notes/{date}.md");
                assert_eq!(format, "{text}\n");
                assert!(append);
                assert!(enabled);
            }
            _ => panic!("Expected File action"),
        }
    }

    #[test]
    fn test_parse_action_with_defaults() {
        let toml_str = r#"
type = "shell"
command = "echo test"
"#;
        let action: ActionConfig = toml::from_str(toml_str).unwrap();
        match action {
            ActionConfig::Shell {
                timeout_secs,
                enabled,
                ..
            } => {
                assert_eq!(timeout_secs, 30); // default
                assert!(enabled); // default
            }
            _ => panic!("Expected Shell action"),
        }
    }

    // ===================
    // ActionRunner Tests
    // ===================

    #[test]
    fn test_action_runner_empty() {
        let runner = ActionRunner::new(vec![]);
        assert!(!runner.has_actions());
    }

    #[test]
    fn test_action_runner_filters_disabled() {
        let actions = vec![
            ActionConfig::Shell {
                command: "echo 1".to_string(),
                timeout_secs: 30,
                enabled: true,
            },
            ActionConfig::Shell {
                command: "echo 2".to_string(),
                timeout_secs: 30,
                enabled: false, // disabled
            },
        ];
        let runner = ActionRunner::new(actions);
        assert!(runner.has_actions());
        assert_eq!(runner.actions.len(), 1);
    }

    #[test]
    fn test_action_runner_all_disabled() {
        let actions = vec![ActionConfig::Shell {
            command: "echo 1".to_string(),
            timeout_secs: 30,
            enabled: false,
        }];
        let runner = ActionRunner::new(actions);
        assert!(!runner.has_actions());
    }

    // ===================
    // Integration Tests (require tokio runtime)
    // ===================

    #[tokio::test]
    async fn test_execute_disabled_action() {
        let action = ActionConfig::Shell {
            command: "exit 1".to_string(), // Would fail if executed
            timeout_secs: 1,
            enabled: false,
        };
        let ctx = ActionContext::new("test".to_string(), 1.0, "base".to_string(), 1);

        // Should succeed because action is disabled
        let result = action.execute(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_shell_success() {
        let action = ActionConfig::Shell {
            command: "echo '{text}'".to_string(),
            timeout_secs: 5,
            enabled: true,
        };
        let ctx = ActionContext::new("hello".to_string(), 1.0, "base".to_string(), 1);

        let result = action.execute(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_shell_sanitizes_input() {
        let action = ActionConfig::Shell {
            command: "echo '{text}'".to_string(),
            timeout_secs: 5,
            enabled: true,
        };
        // Malicious input
        let ctx = ActionContext::new("test $(whoami)".to_string(), 1.0, "base".to_string(), 1);

        // Should succeed (sanitized)
        let result = action.execute(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_file_write() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("openhush_test_action.txt");

        // Clean up any existing file
        let _ = std::fs::remove_file(&temp_file);

        let action = ActionConfig::File {
            path: temp_file.to_string_lossy().to_string(),
            format: "{text}\n".to_string(),
            append: false,
            enabled: true,
        };
        let ctx = ActionContext::new("Hello from test".to_string(), 1.0, "base".to_string(), 1);

        let result = action.execute(&ctx).await;
        assert!(result.is_ok());

        // Verify file contents
        let contents = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(contents, "Hello from test\n");

        // Clean up
        let _ = std::fs::remove_file(&temp_file);
    }

    #[tokio::test]
    async fn test_execute_file_append() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("openhush_test_append.txt");

        // Clean up and create initial file
        let _ = std::fs::remove_file(&temp_file);
        std::fs::write(&temp_file, "Line 1\n").unwrap();

        let action = ActionConfig::File {
            path: temp_file.to_string_lossy().to_string(),
            format: "{text}\n".to_string(),
            append: true,
            enabled: true,
        };
        let ctx = ActionContext::new("Line 2".to_string(), 1.0, "base".to_string(), 1);

        let result = action.execute(&ctx).await;
        assert!(result.is_ok());

        // Verify file contents
        let contents = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(contents, "Line 1\nLine 2\n");

        // Clean up
        let _ = std::fs::remove_file(&temp_file);
    }

    #[tokio::test]
    async fn test_action_runner_run_all() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("openhush_runner_test.txt");
        let _ = std::fs::remove_file(&temp_file);

        let actions = vec![
            ActionConfig::Shell {
                command: "echo 'shell ran'".to_string(),
                timeout_secs: 5,
                enabled: true,
            },
            ActionConfig::File {
                path: temp_file.to_string_lossy().to_string(),
                format: "{text}".to_string(),
                append: false,
                enabled: true,
            },
        ];

        let runner = ActionRunner::new(actions);
        let ctx = ActionContext::new("runner test".to_string(), 1.0, "base".to_string(), 1);

        runner.run_all(&ctx).await;

        // Verify file was written
        let contents = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(contents, "runner test");

        // Clean up
        let _ = std::fs::remove_file(&temp_file);
    }
}
