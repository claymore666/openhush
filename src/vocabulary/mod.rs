//! Custom vocabulary system for domain-specific term replacement.
//!
//! Allows users to define replacements that are applied after Whisper
//! transcription but before LLM correction.
//!
//! # File Format
//!
//! Vocabulary files use TOML format:
//!
//! ```toml
//! # General replacements
//! [replacements]
//! enabled = true
//! case_sensitive = false
//! "gonna" = "going to"
//! "wanna" = "want to"
//!
//! # Medical terms (case-sensitive by default)
//! [medical]
//! enabled = true
//! case_sensitive = true
//! "bp" = "blood pressure"
//! "rx" = "prescription"
//!
//! # Acronyms
//! [acronyms]
//! enabled = true
//! case_sensitive = true
//! "AI" = "artificial intelligence"
//! "ML" = "machine learning"
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Vocabulary-related errors.
#[derive(Error, Debug)]
pub enum VocabularyError {
    #[error("Failed to read vocabulary file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse vocabulary file: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Invalid vocabulary configuration: {0}")]
    InvalidConfig(String),
}

/// A single vocabulary section with its settings and replacements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularySection {
    /// Whether this section is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether replacements are case-sensitive
    #[serde(default)]
    pub case_sensitive: bool,

    /// Replacement mappings (from -> to)
    #[serde(flatten)]
    pub replacements: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

/// Compiled replacement rule for efficient matching.
#[derive(Debug, Clone)]
struct ReplacementRule {
    /// Pattern to match (lowercase if case-insensitive)
    pattern: String,
    /// Original pattern for case-sensitive matching
    original_pattern: String,
    /// Replacement text
    replacement: String,
    /// Whether this rule is case-sensitive
    case_sensitive: bool,
    /// Section name this rule belongs to
    section: String,
}

/// Custom vocabulary manager.
///
/// Handles loading, hot-reloading, and applying vocabulary replacements.
pub struct VocabularyManager {
    /// Path to vocabulary file
    path: PathBuf,
    /// Compiled replacement rules (sorted by pattern length, longest first)
    rules: Arc<RwLock<Vec<ReplacementRule>>>,
    /// Last modification time of the file
    last_modified: Arc<RwLock<Option<std::time::SystemTime>>>,
}

impl VocabularyManager {
    /// Create a new vocabulary manager.
    ///
    /// # Arguments
    /// * `path` - Path to the vocabulary TOML file
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            rules: Arc::new(RwLock::new(Vec::new())),
            last_modified: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the default vocabulary file path.
    pub fn default_path() -> Result<PathBuf, VocabularyError> {
        let config_dir = crate::config::Config::config_dir()
            .map_err(|e| VocabularyError::InvalidConfig(e.to_string()))?;
        Ok(config_dir.join("vocabulary.toml"))
    }

    /// Load vocabulary from file.
    ///
    /// Returns Ok(true) if loaded successfully, Ok(false) if file doesn't exist.
    pub async fn load(&self) -> Result<bool, VocabularyError> {
        if !self.path.exists() {
            debug!("Vocabulary file not found: {}", self.path.display());
            return Ok(false);
        }

        let metadata = std::fs::metadata(&self.path)?;
        let modified = metadata.modified().ok();

        // Check if file has changed
        {
            let last = self.last_modified.read().await;
            if *last == modified {
                return Ok(true); // No changes
            }
        }

        // Read and parse file
        let contents = std::fs::read_to_string(&self.path)?;
        let sections: HashMap<String, VocabularySection> = toml::from_str(&contents)?;

        // Compile rules
        let mut rules = Vec::new();
        let section_count = sections.len();
        for (section_name, section) in sections {
            if !section.enabled {
                debug!("Vocabulary section '{}' is disabled", section_name);
                continue;
            }

            for (pattern, replacement) in &section.replacements {
                // Skip special keys
                if pattern == "enabled" || pattern == "case_sensitive" {
                    continue;
                }

                rules.push(ReplacementRule {
                    pattern: if section.case_sensitive {
                        pattern.clone()
                    } else {
                        pattern.to_lowercase()
                    },
                    original_pattern: pattern.clone(),
                    replacement: replacement.clone(),
                    case_sensitive: section.case_sensitive,
                    section: section_name.clone(),
                });
            }
        }

        // Sort by pattern length (longest first) for correct matching
        rules.sort_by(|a, b| b.pattern.len().cmp(&a.pattern.len()));

        info!(
            "Loaded {} vocabulary rules from {} sections",
            rules.len(),
            section_count
        );

        // Update state
        *self.rules.write().await = rules;
        *self.last_modified.write().await = modified;

        Ok(true)
    }

    /// Check if file has changed and reload if necessary.
    ///
    /// Returns true if reloaded.
    pub async fn check_reload(&self) -> Result<bool, VocabularyError> {
        if !self.path.exists() {
            return Ok(false);
        }

        let metadata = std::fs::metadata(&self.path)?;
        let modified = metadata.modified().ok();

        let needs_reload = {
            let last = self.last_modified.read().await;
            *last != modified
        };

        if needs_reload {
            info!("Vocabulary file changed, reloading...");
            self.load().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Apply vocabulary replacements to text.
    ///
    /// Replacements are applied in order of pattern length (longest first)
    /// to handle overlapping patterns correctly.
    pub async fn apply(&self, text: &str) -> String {
        let rules = self.rules.read().await;
        if rules.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();
        let mut replacements_made = 0;

        for rule in rules.iter() {
            let (new_result, count) = if rule.case_sensitive {
                Self::replace_exact(&result, &rule.pattern, &rule.replacement)
            } else {
                Self::replace_case_insensitive(&result, &rule.pattern, &rule.replacement)
            };
            if count > 0 {
                debug!(
                    "Replaced '{}' -> '{}' ({} times, section: {})",
                    rule.original_pattern, rule.replacement, count, rule.section
                );
                replacements_made += count;
            }
            result = new_result;
        }

        if replacements_made > 0 {
            debug!("Applied {} vocabulary replacements", replacements_made);
        }

        result
    }

    /// Exact (case-sensitive) word boundary replacement.
    fn replace_exact(text: &str, pattern: &str, replacement: &str) -> (String, usize) {
        let mut result = String::new();
        let mut count = 0;
        let mut last_end = 0;

        let chars: Vec<char> = text.chars().collect();
        let pattern_chars: Vec<char> = pattern.chars().collect();

        let mut i = 0;
        while i <= chars.len().saturating_sub(pattern_chars.len()) {
            // Check for word boundary before pattern
            let at_word_start = i == 0 || !chars[i - 1].is_alphanumeric();

            if at_word_start {
                // Check if pattern matches
                let matches = pattern_chars
                    .iter()
                    .enumerate()
                    .all(|(j, &pc)| i + j < chars.len() && chars[i + j] == pc);

                if matches {
                    // Check for word boundary after pattern
                    let end_pos = i + pattern_chars.len();
                    let at_word_end = end_pos >= chars.len() || !chars[end_pos].is_alphanumeric();

                    if at_word_end {
                        // Found a match at word boundary
                        result.push_str(
                            &text[last_end
                                ..text
                                    .char_indices()
                                    .nth(i)
                                    .map(|(idx, _)| idx)
                                    .unwrap_or(text.len())],
                        );
                        result.push_str(replacement);
                        last_end = text
                            .char_indices()
                            .nth(end_pos)
                            .map(|(idx, _)| idx)
                            .unwrap_or(text.len());
                        i = end_pos;
                        count += 1;
                        continue;
                    }
                }
            }
            i += 1;
        }

        result.push_str(&text[last_end..]);
        (result, count)
    }

    /// Case-insensitive word boundary replacement.
    fn replace_case_insensitive(text: &str, pattern: &str, replacement: &str) -> (String, usize) {
        let mut result = String::new();
        let mut count = 0;
        let mut last_end = 0;

        let text_lower = text.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        let chars: Vec<char> = text.chars().collect();
        let text_lower_chars: Vec<char> = text_lower.chars().collect();
        let pattern_chars: Vec<char> = pattern_lower.chars().collect();

        let mut i = 0;
        while i <= chars.len().saturating_sub(pattern_chars.len()) {
            // Check for word boundary before pattern
            let at_word_start = i == 0 || !chars[i - 1].is_alphanumeric();

            if at_word_start {
                // Check if pattern matches (case-insensitive)
                let matches = pattern_chars.iter().enumerate().all(|(j, &pc)| {
                    i + j < text_lower_chars.len() && text_lower_chars[i + j] == pc
                });

                if matches {
                    // Check for word boundary after pattern
                    let end_pos = i + pattern_chars.len();
                    let at_word_end = end_pos >= chars.len() || !chars[end_pos].is_alphanumeric();

                    if at_word_end {
                        // Found a match at word boundary
                        result.push_str(
                            &text[last_end
                                ..text
                                    .char_indices()
                                    .nth(i)
                                    .map(|(idx, _)| idx)
                                    .unwrap_or(text.len())],
                        );
                        result.push_str(replacement);
                        last_end = text
                            .char_indices()
                            .nth(end_pos)
                            .map(|(idx, _)| idx)
                            .unwrap_or(text.len());
                        i = end_pos;
                        count += 1;
                        continue;
                    }
                }
            }
            i += 1;
        }

        result.push_str(&text[last_end..]);
        (result, count)
    }

    /// Get the number of loaded rules.
    pub async fn rule_count(&self) -> usize {
        self.rules.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_vocabulary_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[replacements]
enabled = true
case_sensitive = false
"gonna" = "going to"
"wanna" = "want to"

[medical]
enabled = true
case_sensitive = true
"bp" = "blood pressure"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        assert!(manager.load().await.unwrap());
        assert_eq!(manager.rule_count().await, 3);
    }

    #[tokio::test]
    async fn test_vocabulary_case_insensitive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[replacements]
enabled = true
case_sensitive = false
"gonna" = "going to"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();

        assert_eq!(manager.apply("I'm GONNA do it").await, "I'm going to do it");
        assert_eq!(manager.apply("I'm gonna do it").await, "I'm going to do it");
    }

    #[tokio::test]
    async fn test_vocabulary_case_sensitive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[acronyms]
enabled = true
case_sensitive = true
"AI" = "artificial intelligence"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();

        assert_eq!(
            manager.apply("I love AI").await,
            "I love artificial intelligence"
        );
        // "ai" should not match
        assert_eq!(manager.apply("I love ai").await, "I love ai");
    }

    #[tokio::test]
    async fn test_vocabulary_word_boundaries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[replacements]
enabled = true
case_sensitive = false
"rx" = "prescription"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();

        // Should match at word boundary
        assert_eq!(manager.apply("Take the rx").await, "Take the prescription");

        // Should NOT match inside a word
        assert_eq!(manager.apply("proximal").await, "proximal");
    }

    #[tokio::test]
    async fn test_vocabulary_multi_word() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[phrases]
enabled = true
case_sensitive = false
"blood pressure" = "BP"
"gonna go" = "will go"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();

        assert_eq!(
            manager.apply("Check the blood pressure").await,
            "Check the BP"
        );
        assert_eq!(manager.apply("I'm gonna go now").await, "I'm will go now");
    }

    #[tokio::test]
    async fn test_vocabulary_longest_first() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[test]
enabled = true
case_sensitive = false
"go" = "move"
"gonna go" = "will leave"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();

        // Longer pattern should match first
        assert_eq!(
            manager.apply("I'm gonna go now").await,
            "I'm will leave now"
        );
    }

    #[tokio::test]
    async fn test_vocabulary_disabled_section() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        std::fs::write(
            &path,
            r#"
[enabled_section]
enabled = true
"foo" = "bar"

[disabled_section]
enabled = false
"baz" = "qux"
"#,
        )
        .unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();

        // Only one rule should be loaded (from enabled section)
        assert_eq!(manager.rule_count().await, 1);

        assert_eq!(manager.apply("foo baz").await, "bar baz");
    }

    #[tokio::test]
    async fn test_vocabulary_no_file() {
        let manager = VocabularyManager::new(PathBuf::from("/nonexistent/path/vocabulary.toml"));
        assert!(!manager.load().await.unwrap()); // Returns false, not error
        assert_eq!(manager.rule_count().await, 0);
    }

    #[tokio::test]
    #[ignore = "flaky in CI/coverage builds due to instrumentation overhead"]
    async fn test_vocabulary_performance() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vocabulary.toml");

        // Create 1000 rules
        let mut content = String::from("[rules]\nenabled = true\ncase_sensitive = false\n");
        for i in 0..1000 {
            content.push_str(&format!("\"word{}\" = \"replacement{}\"\n", i, i));
        }
        std::fs::write(&path, content).unwrap();

        let manager = VocabularyManager::new(path);
        manager.load().await.unwrap();
        assert_eq!(manager.rule_count().await, 1000);

        // Test performance - should complete in under 10ms
        let text = "word0 word500 word999 other text here";
        let start = std::time::Instant::now();
        let _result = manager.apply(text).await;
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 10,
            "Vocabulary replacement took {}ms (target: <10ms)",
            elapsed.as_millis()
        );
    }
}
