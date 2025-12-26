//! System tray icon and menu using D-Bus StatusNotifierItem.
//!
//! Uses the ksni crate for cross-desktop tray support (KDE, GNOME with extensions, etc.)
//! without requiring GTK dependencies.

use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
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
