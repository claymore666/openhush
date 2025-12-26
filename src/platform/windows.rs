//! Windows platform implementation
//!
//! Uses arboard for clipboard, enigo for keyboard simulation (SendInput),
//! and notify-rust for toast notifications.
//!
//! The crates used provide cross-platform APIs that work on Windows.

use super::{
    AudioFeedback, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError, SystemTray,
    TextOutput, TrayMenuEvent, TrayStatus,
};
use arboard::Clipboard;
use std::sync::Mutex;

pub struct WindowsPlatform {
    clipboard: Mutex<Option<Clipboard>>,
}

impl WindowsPlatform {
    pub fn new() -> Result<Self, PlatformError> {
        let clipboard = Clipboard::new().map(Some).unwrap_or_else(|_| None);
        Ok(Self {
            clipboard: Mutex::new(clipboard),
        })
    }

    /// Paste text using enigo direct typing (instant)
    fn paste_via_enigo(&self, text: &str) -> Result<(), PlatformError> {
        use enigo::{Enigo, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| PlatformError::Paste(format!("Failed to init enigo: {:?}", e)))?;

        // Small delay to ensure target window has focus
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Type text directly - instant, no clipboard modification
        enigo
            .text(text)
            .map_err(|e| PlatformError::Paste(format!("Failed to type text: {:?}", e)))?;

        Ok(())
    }
}

impl Default for WindowsPlatform {
    /// Creates a WindowsPlatform with default settings.
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: WindowsPlatform initialization failed: {}. Using fallback.",
                e
            );
            Self {
                clipboard: Mutex::new(None),
            }
        })
    }
}

impl HotkeyHandler for WindowsPlatform {
    fn start(&mut self, _key: &str) -> Result<(), PlatformError> {
        // Hotkeys are handled globally by the rdev crate in daemon.rs
        // This is a no-op on Windows as rdev works cross-platform
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

impl TextOutput for WindowsPlatform {
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
        self.paste_via_enigo(text)
    }
}

impl Notifier for WindowsPlatform {
    fn notify(&self, title: &str, body: &str) -> Result<(), PlatformError> {
        // Use notify-rust which supports Windows toast notifications
        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .appname("OpenHush")
            .show()
            .map_err(|e| PlatformError::Notification(e.to_string()))?;

        Ok(())
    }
}

impl AudioFeedback for WindowsPlatform {
    fn play_start_sound(&self) -> Result<(), PlatformError> {
        // Play Windows system sound using PowerShell
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[System.Media.SystemSounds]::Asterisk.Play()",
            ])
            .spawn()
            .ok();
        Ok(())
    }

    fn play_stop_sound(&self) -> Result<(), PlatformError> {
        // Play a different Windows system sound
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[System.Media.SystemSounds]::Beep.Play()",
            ])
            .spawn()
            .ok();
        Ok(())
    }
}

impl Platform for WindowsPlatform {
    fn display_server(&self) -> &str {
        "Windows"
    }

    fn is_tty(&self) -> bool {
        false
    }
}

/// Windows system tray implementation.
///
/// Uses the tray-icon or notify-rust crate for system tray integration.
/// Note: Full implementation requires running on the main thread.
pub struct WindowsSystemTray {
    status: TrayStatus,
}

impl SystemTray for WindowsSystemTray {
    fn new() -> Result<Self, PlatformError> {
        // System tray will be initialized by the main app
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
        // Windows always has system tray support
        true
    }
}
