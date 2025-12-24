//! Tray icon utilities.
//!
//! Provides icon names and utilities for system tray icons.
//! Uses freedesktop standard icon names for cross-desktop compatibility.

/// Default icon name for the tray (idle state)
pub const ICON_IDLE: &str = "audio-input-microphone";

/// Icon name for recording state
pub const ICON_RECORDING: &str = "media-record";

/// Icon name for processing state
pub const ICON_PROCESSING: &str = "view-refresh";

/// Icon name for error state
pub const ICON_ERROR: &str = "dialog-error";

/// Create the tray icon (returns icon name for ksni)
///
/// ksni uses freedesktop icon names, so we return the standard
/// microphone icon name.
pub fn create_icon() -> Result<String, String> {
    Ok(ICON_IDLE.to_string())
}

/// Create a recording indicator icon name
#[allow(dead_code)]
pub fn create_recording_icon() -> Result<String, String> {
    Ok(ICON_RECORDING.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_icon() {
        let icon = create_icon();
        assert!(icon.is_ok());
        assert_eq!(icon.unwrap(), ICON_IDLE);
    }

    #[test]
    fn test_create_recording_icon() {
        let icon = create_recording_icon();
        assert!(icon.is_ok());
        assert_eq!(icon.unwrap(), ICON_RECORDING);
    }

    #[test]
    fn test_icon_names_not_empty() {
        assert!(!ICON_IDLE.is_empty());
        assert!(!ICON_RECORDING.is_empty());
        assert!(!ICON_PROCESSING.is_empty());
        assert!(!ICON_ERROR.is_empty());
    }
}
