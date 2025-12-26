//! Tray icon utilities.
//!
//! Provides icon names and utilities for system tray icons.
//! Uses freedesktop standard icon names for Linux, and embedded RGBA data for Windows.

#![allow(dead_code)]

/// Default icon name for the tray (idle state) - Linux freedesktop
pub const ICON_IDLE: &str = "audio-input-microphone";

/// Icon name for recording state
pub const ICON_RECORDING: &str = "media-record";

/// Icon name for processing state
pub const ICON_PROCESSING: &str = "view-refresh";

/// Icon name for error state
pub const ICON_ERROR: &str = "dialog-error";

/// Icon dimensions (32x32)
pub const ICON_WIDTH: u32 = 32;
pub const ICON_HEIGHT: u32 = 32;

/// Embedded RGBA icon data (32x32 microphone icon)
/// Simple blue circle with white microphone shape
pub const ICON_DATA: &[u8] = &{
    const SIZE: usize = 32 * 32 * 4;
    let mut data = [0u8; SIZE];

    // Create a simple icon: blue circle with white center
    let mut i = 0;
    while i < 32 {
        let mut j = 0;
        while j < 32 {
            let idx = (i * 32 + j) * 4;
            let cx = 16i32;
            let cy = 16i32;
            let dx = j as i32 - cx;
            let dy = i as i32 - cy;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq <= 196 {
                // Inside circle (radius 14)
                if dist_sq <= 64 {
                    // Inner white area (microphone body, radius 8)
                    data[idx] = 255; // R
                    data[idx + 1] = 255; // G
                    data[idx + 2] = 255; // B
                    data[idx + 3] = 255; // A
                } else {
                    // Blue ring
                    data[idx] = 66; // R
                    data[idx + 1] = 133; // G
                    data[idx + 2] = 244; // B (Google blue)
                    data[idx + 3] = 255; // A
                }
            } else {
                // Transparent outside
                data[idx] = 0;
                data[idx + 1] = 0;
                data[idx + 2] = 0;
                data[idx + 3] = 0;
            }
            j += 1;
        }
        i += 1;
    }
    data
};

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
