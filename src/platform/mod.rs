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
