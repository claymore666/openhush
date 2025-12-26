//! Platform abstraction layer for cross-platform functionality.
//!
//! This module provides traits and implementations for platform-specific operations:
//! - Hotkey handling
//! - Text pasting/typing
//! - Notifications
//! - Audio feedback

#![allow(dead_code)]

use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::LinuxPlatform as CurrentPlatform;
#[cfg(target_os = "macos")]
pub use macos::MacOSPlatform as CurrentPlatform;
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform as CurrentPlatform;

// System tray types (currently unused, but available for future use)
#[allow(unused_imports)]
#[cfg(target_os = "linux")]
pub use linux::LinuxSystemTray as CurrentSystemTray;
#[allow(unused_imports)]
#[cfg(target_os = "macos")]
pub use macos::MacOSSystemTray as CurrentSystemTray;
#[allow(unused_imports)]
#[cfg(target_os = "windows")]
pub use windows::WindowsSystemTray as CurrentSystemTray;

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Hotkey error: {0}")]
    Hotkey(String),

    #[error("Paste error: {0}")]
    Paste(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Notification error: {0}")]
    Notification(String),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("Tray error: {0}")]
    Tray(String),

    #[error("Platform not supported: {0}")]
    NotSupported(String),

    #[error("Accessibility permission required: {0}")]
    Accessibility(String),

    #[error("{0}")]
    Other(String),
}

/// Hotkey events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// Trait for platform-specific hotkey handling
pub trait HotkeyHandler: Send + Sync {
    /// Start listening for the configured hotkey
    fn start(&mut self, key: &str) -> Result<(), PlatformError>;

    /// Stop listening
    fn stop(&mut self) -> Result<(), PlatformError>;

    /// Poll for hotkey events (non-blocking)
    fn poll(&mut self) -> Option<HotkeyEvent>;
}

/// Trait for platform-specific text output
pub trait TextOutput: Send + Sync {
    /// Copy text to clipboard
    fn copy_to_clipboard(&self, text: &str) -> Result<(), PlatformError>;

    /// Paste/type text at cursor position
    fn paste_text(&self, text: &str) -> Result<(), PlatformError>;
}

/// Trait for platform-specific notifications
pub trait Notifier: Send + Sync {
    /// Show a notification
    fn notify(&self, title: &str, body: &str) -> Result<(), PlatformError>;
}

/// Trait for platform-specific audio feedback
pub trait AudioFeedback: Send + Sync {
    /// Play a beep sound for recording start
    fn play_start_sound(&self) -> Result<(), PlatformError>;

    /// Play a beep sound for recording stop
    fn play_stop_sound(&self) -> Result<(), PlatformError>;
}

/// System tray status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    /// Idle, ready to record
    Idle,
    /// Currently recording
    Recording,
    /// Processing transcription
    Processing,
    /// Error state
    Error,
}

/// Events from the system tray
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayMenuEvent {
    /// User clicked "Preferences..."
    ShowPreferences,
    /// User clicked "Quit"
    Quit,
}

/// Trait for platform-specific system tray
pub trait SystemTray {
    /// Create a new system tray icon
    fn new() -> Result<Self, PlatformError>
    where
        Self: Sized;

    /// Update the tray status (changes icon/tooltip)
    fn set_status(&mut self, status: TrayStatus);

    /// Poll for menu events (non-blocking)
    fn poll_event(&mut self) -> Option<TrayMenuEvent>;

    /// Check if tray is supported on this platform
    fn is_supported() -> bool
    where
        Self: Sized;
}

/// Combined platform interface
pub trait Platform: HotkeyHandler + TextOutput + Notifier + AudioFeedback {
    /// Get the display server type (X11, Wayland, Windows, macOS)
    fn display_server(&self) -> &str;

    /// Check if running in a TTY (no GUI)
    fn is_tty(&self) -> bool;
}

/// Detect the current display environment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Windows,
    MacOS,
    Tty,
    Unknown,
}

impl DisplayServer {
    /// Detect the current display server
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                return DisplayServer::Wayland;
            }
            if std::env::var("DISPLAY").is_ok() {
                return DisplayServer::X11;
            }
            // Check if we're in a TTY
            if std::env::var("TERM").is_ok() && std::env::var("DISPLAY").is_err() {
                return DisplayServer::Tty;
            }
            DisplayServer::Unknown
        }

        #[cfg(target_os = "macos")]
        {
            DisplayServer::MacOS
        }

        #[cfg(target_os = "windows")]
        {
            DisplayServer::Windows
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            DisplayServer::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // DisplayServer Tests
    // ===================

    #[test]
    fn test_display_server_equality() {
        assert_eq!(DisplayServer::X11, DisplayServer::X11);
        assert_eq!(DisplayServer::Wayland, DisplayServer::Wayland);
        assert_ne!(DisplayServer::X11, DisplayServer::Wayland);
    }

    #[test]
    fn test_display_server_debug() {
        let server = DisplayServer::X11;
        let debug = format!("{:?}", server);
        assert_eq!(debug, "X11");

        let server = DisplayServer::Wayland;
        let debug = format!("{:?}", server);
        assert_eq!(debug, "Wayland");
    }

    #[test]
    fn test_display_server_clone() {
        let server = DisplayServer::MacOS;
        let cloned = server;
        assert_eq!(server, cloned);
    }

    #[test]
    fn test_display_server_all_variants() {
        let _x11 = DisplayServer::X11;
        let _wayland = DisplayServer::Wayland;
        let _windows = DisplayServer::Windows;
        let _macos = DisplayServer::MacOS;
        let _tty = DisplayServer::Tty;
        let _unknown = DisplayServer::Unknown;
    }

    // DisplayServer::detect() depends on environment variables,
    // so we can only test that it returns a valid variant
    #[test]
    fn test_display_server_detect_returns_valid() {
        let server = DisplayServer::detect();
        // Just verify it's one of the valid variants
        match server {
            DisplayServer::X11
            | DisplayServer::Wayland
            | DisplayServer::Windows
            | DisplayServer::MacOS
            | DisplayServer::Tty
            | DisplayServer::Unknown => {}
        }
    }

    // ===================
    // HotkeyEvent Tests
    // ===================

    #[test]
    fn test_hotkey_event_equality() {
        assert_eq!(HotkeyEvent::Pressed, HotkeyEvent::Pressed);
        assert_eq!(HotkeyEvent::Released, HotkeyEvent::Released);
        assert_ne!(HotkeyEvent::Pressed, HotkeyEvent::Released);
    }

    #[test]
    fn test_hotkey_event_debug() {
        assert_eq!(format!("{:?}", HotkeyEvent::Pressed), "Pressed");
        assert_eq!(format!("{:?}", HotkeyEvent::Released), "Released");
    }

    #[test]
    fn test_hotkey_event_clone() {
        let event = HotkeyEvent::Pressed;
        let cloned = event;
        assert_eq!(event, cloned);
    }

    // ===================
    // PlatformError Tests
    // ===================

    #[test]
    fn test_platform_error_display() {
        let err = PlatformError::Hotkey("key error".to_string());
        assert!(err.to_string().contains("Hotkey error"));
        assert!(err.to_string().contains("key error"));

        let err = PlatformError::Paste("paste failed".to_string());
        assert!(err.to_string().contains("Paste error"));

        let err = PlatformError::Clipboard("copy failed".to_string());
        assert!(err.to_string().contains("Clipboard error"));

        let err = PlatformError::Notification("notify failed".to_string());
        assert!(err.to_string().contains("Notification error"));

        let err = PlatformError::Audio("audio failed".to_string());
        assert!(err.to_string().contains("Audio error"));

        let err = PlatformError::Tray("tray failed".to_string());
        assert!(err.to_string().contains("Tray error"));

        let err = PlatformError::NotSupported("feature".to_string());
        assert!(err.to_string().contains("not supported"));

        let err = PlatformError::Accessibility("permission denied".to_string());
        assert!(err.to_string().contains("Accessibility"));

        let err = PlatformError::Other("something else".to_string());
        assert!(err.to_string().contains("something else"));
    }

    #[test]
    fn test_platform_error_debug() {
        let err = PlatformError::Hotkey("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Hotkey"));
    }

    // ===================
    // TrayStatus Tests
    // ===================

    #[test]
    fn test_tray_status_equality() {
        assert_eq!(TrayStatus::Idle, TrayStatus::Idle);
        assert_eq!(TrayStatus::Recording, TrayStatus::Recording);
        assert_ne!(TrayStatus::Idle, TrayStatus::Recording);
    }

    #[test]
    fn test_tray_status_debug() {
        assert_eq!(format!("{:?}", TrayStatus::Idle), "Idle");
        assert_eq!(format!("{:?}", TrayStatus::Recording), "Recording");
        assert_eq!(format!("{:?}", TrayStatus::Processing), "Processing");
        assert_eq!(format!("{:?}", TrayStatus::Error), "Error");
    }

    #[test]
    fn test_tray_status_clone() {
        let status = TrayStatus::Recording;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    // ===================
    // TrayMenuEvent Tests
    // ===================

    #[test]
    fn test_tray_menu_event_equality() {
        assert_eq!(
            TrayMenuEvent::ShowPreferences,
            TrayMenuEvent::ShowPreferences
        );
        assert_eq!(TrayMenuEvent::Quit, TrayMenuEvent::Quit);
        assert_ne!(TrayMenuEvent::ShowPreferences, TrayMenuEvent::Quit);
    }

    #[test]
    fn test_tray_menu_event_debug() {
        assert_eq!(
            format!("{:?}", TrayMenuEvent::ShowPreferences),
            "ShowPreferences"
        );
        assert_eq!(format!("{:?}", TrayMenuEvent::Quit), "Quit");
    }

    #[test]
    fn test_tray_menu_event_clone() {
        let event = TrayMenuEvent::Quit;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
