//! Output handling: clipboard, paste, and post-transcription actions.

pub mod actions;
pub mod clipboard;
pub mod paste;

pub use actions::{ActionContext, ActionRunner};
pub use clipboard::{copy_to_clipboard, ClipboardError};
pub use paste::{paste_text, PasteError};

use crate::config::OutputConfig;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("Clipboard error: {0}")]
    Clipboard(#[from] ClipboardError),

    #[error("Paste error: {0}")]
    Paste(#[from] PasteError),
}

/// Output handler that manages clipboard and paste operations
pub struct OutputHandler {
    clipboard_enabled: bool,
    paste_enabled: bool,
}

impl OutputHandler {
    /// Create a new output handler from config
    pub fn new(config: &OutputConfig) -> Self {
        Self {
            clipboard_enabled: config.clipboard,
            paste_enabled: config.paste,
        }
    }

    /// Output text according to configuration
    ///
    /// This will:
    /// 1. Copy to clipboard (if enabled)
    /// 2. Paste at cursor (if enabled)
    pub fn output(&self, text: &str) -> Result<(), OutputError> {
        if text.is_empty() {
            info!("Empty text, skipping output");
            return Ok(());
        }

        // Copy to clipboard first (always succeeds even if paste fails)
        if self.clipboard_enabled {
            copy_to_clipboard(text)?;
        }

        // Then paste at cursor
        if self.paste_enabled {
            paste_text(text)?;
        }

        Ok(())
    }

    /// Copy text to clipboard only
    #[allow(dead_code)]
    pub fn copy_only(&self, text: &str) -> Result<(), OutputError> {
        copy_to_clipboard(text)?;
        Ok(())
    }

    /// Paste text at cursor only (assumes text is already in clipboard)
    #[allow(dead_code)]
    pub fn paste_only(&self, text: &str) -> Result<(), OutputError> {
        paste_text(text)?;
        Ok(())
    }
}
