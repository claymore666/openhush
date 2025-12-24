//! macOS platform implementation
//!
//! Uses arboard for clipboard, enigo for text input, and notify-rust for notifications.
//! These crates provide cross-platform APIs that work on macOS.
//!
//! Note: Accessibility permissions are required for keyboard simulation.
//! The user will be prompted by macOS when first using the app.

use super::{
    AudioFeedback, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError, SystemTray,
    TextOutput, TrayMenuEvent, TrayStatus,
};
use arboard::Clipboard;
use std::sync::Mutex;

pub struct MacOSPlatform {
    clipboard: Mutex<Option<Clipboard>>,
}

impl MacOSPlatform {
    pub fn new() -> Result<Self, PlatformError> {
        let clipboard = Clipboard::new().map(Some).unwrap_or_else(|_| None);
        Ok(Self {
            clipboard: Mutex::new(clipboard),
        })
    }

    /// Paste text using clipboard + Cmd+V simulation
    fn paste_via_clipboard(&self, text: &str) -> Result<(), PlatformError> {
        // Copy to clipboard first
        self.copy_to_clipboard(text)?;

        // Simulate Cmd+V using osascript (AppleScript)
        // This is more reliable than enigo for cross-application pasting
        std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to keystroke \"v\" using command down",
            ])
            .status()
            .map_err(|e| PlatformError::Paste(format!("osascript failed: {}", e)))?;

        Ok(())
    }
}

impl Default for MacOSPlatform {
    /// Creates a MacOSPlatform with default settings.
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: MacOSPlatform initialization failed: {}. Using fallback.",
                e
            );
            Self {
                clipboard: Mutex::new(None),
            }
        })
    }
}

impl HotkeyHandler for MacOSPlatform {
    fn start(&mut self, _key: &str) -> Result<(), PlatformError> {
        // Hotkeys are handled globally by the rdev crate in daemon.rs
        // This is a no-op on macOS as rdev works cross-platform
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn poll(&mut self) -> Option<HotkeyEvent> {
        // Polling is handled by rdev callback
        None
    }
}

impl TextOutput for MacOSPlatform {
    fn copy_to_clipboard(&self, text: &str) -> Result<(), PlatformError> {
        let mut guard = self
            .clipboard
            .lock()
            .map_err(|_| PlatformError::Clipboard("Clipboard mutex poisoned".into()))?;

        if let Some(ref mut clipboard) = *guard {
            clipboard
                .set_text(text)
                .map_err(|e| PlatformError::Clipboard(e.to_string()))?;
            Ok(())
        } else {
            Err(PlatformError::Clipboard("Clipboard not available".into()))
        }
    }

    fn paste_text(&self, text: &str) -> Result<(), PlatformError> {
        self.paste_via_clipboard(text)
    }
}

impl Notifier for MacOSPlatform {
    fn notify(&self, title: &str, body: &str) -> Result<(), PlatformError> {
        // Use notify-rust which supports macOS notifications
        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .appname("OpenHush")
            .show()
            .map_err(|e| PlatformError::Notification(e.to_string()))?;

        Ok(())
    }
}

impl AudioFeedback for MacOSPlatform {
    fn play_start_sound(&self) -> Result<(), PlatformError> {
        // Play system sound using afplay
        // "Tink" is a subtle, pleasant sound
        std::process::Command::new("afplay")
            .args(["/System/Library/Sounds/Tink.aiff"])
            .spawn()
            .ok();
        Ok(())
    }

    fn play_stop_sound(&self) -> Result<(), PlatformError> {
        // Play a different system sound
        std::process::Command::new("afplay")
            .args(["/System/Library/Sounds/Pop.aiff"])
            .spawn()
            .ok();
        Ok(())
    }
}

impl Platform for MacOSPlatform {
    fn display_server(&self) -> &str {
        "macOS"
    }

    fn is_tty(&self) -> bool {
        false
    }
}

/// macOS system tray (menu bar) implementation.
///
/// Uses the tray-icon crate for menu bar integration.
/// Note: Full implementation requires running on the main thread.
pub struct MacOSSystemTray {
    status: TrayStatus,
}

impl SystemTray for MacOSSystemTray {
    fn new() -> Result<Self, PlatformError> {
        // Menu bar integration will be initialized by the main app
        Ok(Self {
            status: TrayStatus::Idle,
        })
    }

    fn set_status(&mut self, status: TrayStatus) {
        self.status = status;
        // Status updates are handled by the tray manager
    }

    fn poll_event(&mut self) -> Option<TrayMenuEvent> {
        // Events are handled by the tray manager
        None
    }

    fn is_supported() -> bool {
        // macOS always has menu bar support
        true
    }
}
