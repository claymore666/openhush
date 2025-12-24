//! Text paste/typing using enigo.

use enigo::{Enigo, Keyboard, Settings};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum PasteError {
    #[error("Failed to initialize input simulator: {0}")]
    InitFailed(String),

    #[error("Failed to type text: {0}")]
    TypeFailed(String),

    #[error("Paste method not available: {0}")]
    MethodNotAvailable(String),
}

/// Available paste methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasteMethod {
    /// Type text character by character using enigo
    #[default]
    Type,
    /// Use Ctrl+V to paste from clipboard
    #[allow(dead_code)]
    CtrlV,
    /// Use xdotool command (Linux X11 fallback)
    #[allow(dead_code)]
    Xdotool,
}

/// Paste text at the current cursor position
///
/// Uses enigo to simulate keyboard input.
pub fn paste_text(text: &str) -> Result<(), PasteError> {
    paste_text_with_method(text, PasteMethod::Type)
}

/// Paste text using a specific method
pub fn paste_text_with_method(text: &str, method: PasteMethod) -> Result<(), PasteError> {
    match method {
        PasteMethod::Type => paste_by_typing(text),
        PasteMethod::CtrlV => paste_by_ctrl_v(),
        PasteMethod::Xdotool => paste_by_xdotool(text),
    }
}

/// Type text character by character
fn paste_by_typing(text: &str) -> Result<(), PasteError> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| PasteError::InitFailed(format!("{:?}", e)))?;

    debug!("Typing {} characters", text.len());

    // Small delay before typing to ensure window has focus
    thread::sleep(Duration::from_millis(50));

    // Type the text
    enigo
        .text(text)
        .map_err(|e| PasteError::TypeFailed(format!("{:?}", e)))?;

    info!("Text typed at cursor ({} chars)", text.len());

    Ok(())
}

/// Paste using Ctrl+V (requires text to be in clipboard)
fn paste_by_ctrl_v() -> Result<(), PasteError> {
    use enigo::Key;

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| PasteError::InitFailed(format!("{:?}", e)))?;

    // Small delay before pasting
    thread::sleep(Duration::from_millis(50));

    // Press Ctrl+V
    enigo
        .key(Key::Control, enigo::Direction::Press)
        .map_err(|e| PasteError::TypeFailed(format!("{:?}", e)))?;

    enigo
        .key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| PasteError::TypeFailed(format!("{:?}", e)))?;

    enigo
        .key(Key::Control, enigo::Direction::Release)
        .map_err(|e| PasteError::TypeFailed(format!("{:?}", e)))?;

    info!("Pasted from clipboard (Ctrl+V)");

    Ok(())
}

/// Paste using xdotool command (Linux X11 fallback)
fn paste_by_xdotool(text: &str) -> Result<(), PasteError> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        // Check if xdotool is available
        let xdotool_available = Command::new("which")
            .arg("xdotool")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if !xdotool_available {
            return Err(PasteError::MethodNotAvailable(
                "xdotool not installed. Install with: sudo apt install xdotool".into(),
            ));
        }

        // Use xdotool to type the text
        let status = Command::new("xdotool")
            .arg("type")
            .arg("--clearmodifiers")
            .arg("--")
            .arg(text)
            .status()
            .map_err(|e| PasteError::TypeFailed(e.to_string()))?;

        if !status.success() {
            return Err(PasteError::TypeFailed("xdotool returned non-zero".into()));
        }

        info!("Text typed using xdotool ({} chars)", text.len());
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = text;
        Err(PasteError::MethodNotAvailable(
            "xdotool is only available on Linux".into(),
        ))
    }
}

/// Detect the best paste method for the current environment
#[allow(dead_code)]
pub fn detect_paste_method() -> PasteMethod {
    // For now, default to typing
    // In the future, we could detect X11 vs Wayland and choose appropriately
    PasteMethod::Type
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // PasteMethod Tests
    // ===================

    #[test]
    fn test_paste_method_default() {
        assert_eq!(PasteMethod::default(), PasteMethod::Type);
    }

    #[test]
    fn test_paste_method_equality() {
        assert_eq!(PasteMethod::Type, PasteMethod::Type);
        assert_eq!(PasteMethod::CtrlV, PasteMethod::CtrlV);
        assert_eq!(PasteMethod::Xdotool, PasteMethod::Xdotool);
        assert_ne!(PasteMethod::Type, PasteMethod::CtrlV);
    }

    #[test]
    fn test_paste_method_debug() {
        assert_eq!(format!("{:?}", PasteMethod::Type), "Type");
        assert_eq!(format!("{:?}", PasteMethod::CtrlV), "CtrlV");
        assert_eq!(format!("{:?}", PasteMethod::Xdotool), "Xdotool");
    }

    #[test]
    fn test_paste_method_clone() {
        let method = PasteMethod::Type;
        let cloned = method;
        assert_eq!(method, cloned);
    }

    // ===================
    // detect_paste_method Tests
    // ===================

    #[test]
    fn test_detect_paste_method() {
        // Currently always returns Type
        let method = detect_paste_method();
        assert_eq!(method, PasteMethod::Type);
    }

    // ===================
    // PasteError Tests
    // ===================

    #[test]
    fn test_paste_error_display() {
        let err = PasteError::InitFailed("init error".to_string());
        assert!(err.to_string().contains("initialize"));
        assert!(err.to_string().contains("init error"));

        let err = PasteError::TypeFailed("type error".to_string());
        assert!(err.to_string().contains("type text"));

        let err = PasteError::MethodNotAvailable("xdotool".to_string());
        assert!(err.to_string().contains("not available"));
    }

    #[test]
    fn test_paste_error_debug() {
        let err = PasteError::InitFailed("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InitFailed"));
    }

    // ===================
    // Xdotool Platform Tests
    // ===================

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_xdotool_not_available_non_linux() {
        let result = paste_by_xdotool("test");
        assert!(result.is_err());
        match result {
            Err(PasteError::MethodNotAvailable(msg)) => {
                assert!(msg.contains("Linux"));
            }
            _ => panic!("Expected MethodNotAvailable error"),
        }
    }
}
