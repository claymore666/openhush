//! System tray icon and menu using D-Bus StatusNotifierItem.
//!
//! Uses the ksni crate for cross-desktop tray support (KDE, GNOME with extensions, etc.)
//! without requiring GTK dependencies.

use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(target_os = "windows")]
pub use windows::*;

mod icon;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum TrayError {
    #[error("Failed to create tray icon: {0}")]
    IconCreation(String),

    #[error("Failed to create menu: {0}")]
    MenuCreation(String),

    #[error("Failed to build tray: {0}")]
    TrayBuild(String),

    #[error("System tray not supported on this platform")]
    NotSupported,

    #[error("D-Bus error: {0}")]
    DBus(String),
}

/// Events from the system tray menu
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// User clicked "Preferences..."
    ShowPreferences,
    /// User clicked "Quit"
    Quit,
    /// Status item was clicked (informational)
    StatusClicked,
}

/// Status for tray icon display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TrayStatus {
    Idle,
    Recording,
    Processing,
    Error,
}

impl TrayStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrayStatus::Idle => "Status: Idle",
            TrayStatus::Recording => "Status: Recording...",
            TrayStatus::Processing => "Status: Processing...",
            TrayStatus::Error => "Status: Error",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            TrayStatus::Idle => "audio-input-microphone",
            TrayStatus::Recording => "media-record",
            TrayStatus::Processing => "view-refresh",
            TrayStatus::Error => "dialog-error",
        }
    }
}

/// Check if system tray is likely to be supported
#[allow(dead_code)]
pub fn is_tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check for D-Bus session bus
        std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // TrayStatus Tests
    // ===================

    #[test]
    fn test_tray_status_as_str() {
        assert_eq!(TrayStatus::Idle.as_str(), "Status: Idle");
        assert_eq!(TrayStatus::Recording.as_str(), "Status: Recording...");
        assert_eq!(TrayStatus::Processing.as_str(), "Status: Processing...");
        assert_eq!(TrayStatus::Error.as_str(), "Status: Error");
    }

    #[test]
    fn test_tray_status_icon_name() {
        assert_eq!(TrayStatus::Idle.icon_name(), "audio-input-microphone");
        assert_eq!(TrayStatus::Recording.icon_name(), "media-record");
        assert_eq!(TrayStatus::Processing.icon_name(), "view-refresh");
        assert_eq!(TrayStatus::Error.icon_name(), "dialog-error");
    }

    #[test]
    fn test_tray_status_equality() {
        assert_eq!(TrayStatus::Idle, TrayStatus::Idle);
        assert_ne!(TrayStatus::Idle, TrayStatus::Recording);
    }

    #[test]
    fn test_tray_status_clone() {
        let status = TrayStatus::Recording;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn test_tray_status_debug() {
        assert_eq!(format!("{:?}", TrayStatus::Idle), "Idle");
        assert_eq!(format!("{:?}", TrayStatus::Recording), "Recording");
    }

    // ===================
    // TrayEvent Tests
    // ===================

    #[test]
    fn test_tray_event_debug() {
        assert_eq!(
            format!("{:?}", TrayEvent::ShowPreferences),
            "ShowPreferences"
        );
        assert_eq!(format!("{:?}", TrayEvent::Quit), "Quit");
        assert_eq!(format!("{:?}", TrayEvent::StatusClicked), "StatusClicked");
    }

    #[test]
    fn test_tray_event_clone() {
        let event = TrayEvent::Quit;
        let cloned = event.clone();
        assert!(matches!(cloned, TrayEvent::Quit));
    }

    // ===================
    // TrayError Tests
    // ===================

    #[test]
    fn test_tray_error_display() {
        let err = TrayError::IconCreation("test".into());
        assert!(err.to_string().contains("Failed to create tray icon"));

        let err = TrayError::MenuCreation("test".into());
        assert!(err.to_string().contains("Failed to create menu"));

        let err = TrayError::TrayBuild("test".into());
        assert!(err.to_string().contains("Failed to build tray"));

        let err = TrayError::NotSupported;
        assert!(err.to_string().contains("not supported"));

        let err = TrayError::DBus("test".into());
        assert!(err.to_string().contains("D-Bus error"));
    }

    // ===================
    // is_tray_supported Tests
    // ===================

    #[test]
    fn test_is_tray_supported_returns_bool() {
        // Just verify it returns a boolean without panicking
        let _ = is_tray_supported();
    }
}
