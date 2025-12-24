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

    #[error("Platform not supported: {0}")]
    NotSupported(String),
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

        let err = PlatformError::NotSupported("feature".to_string());
        assert!(err.to_string().contains("not supported"));
    }

    #[test]
    fn test_platform_error_debug() {
        let err = PlatformError::Hotkey("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Hotkey"));
    }
}
