//! Clipboard operations using arboard.

use arboard::Clipboard;
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ClipboardError {
    #[error("Failed to access clipboard: {0}")]
    AccessFailed(String),

    #[error("Failed to set clipboard content: {0}")]
    SetFailed(String),

    #[error("Failed to get clipboard content: {0}")]
    #[allow(dead_code)]
    GetFailed(String),
}

/// Copy text to the system clipboard
pub fn copy_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    let mut clipboard =
        Clipboard::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::SetFailed(e.to_string()))?;

    debug!("Copied {} characters to clipboard", text.len());
    info!("Text copied to clipboard");

    Ok(())
}

/// Get text from the system clipboard
#[allow(dead_code)]
pub fn get_from_clipboard() -> Result<String, ClipboardError> {
    let mut clipboard =
        Clipboard::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    clipboard
        .get_text()
        .map_err(|e| ClipboardError::GetFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires display server"]
    fn test_clipboard_roundtrip() {
        let text = "Hello, OpenHush!";
        copy_to_clipboard(text).unwrap();
        let result = get_from_clipboard().unwrap();
        assert_eq!(result, text);
    }
}
