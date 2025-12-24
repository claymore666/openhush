//! Linux platform implementation (X11, Wayland, TTY)

use super::{
    AudioFeedback, DisplayServer, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError,
    SystemTray, TextOutput, TrayMenuEvent, TrayStatus,
};
use arboard::Clipboard;
use std::process::Command;
use std::sync::Mutex;

pub struct LinuxPlatform {
    display_server: DisplayServer,
    clipboard: Mutex<Option<Clipboard>>,
}

impl LinuxPlatform {
    pub fn new() -> Result<Self, PlatformError> {
        let display_server = DisplayServer::detect();

        let clipboard = Clipboard::new().map(Some).unwrap_or_else(|_| None);

        Ok(Self {
            display_server,
            clipboard: Mutex::new(clipboard),
        })
    }

    /// Paste using wtype (Wayland)
    fn paste_wayland(&self, text: &str) -> Result<(), PlatformError> {
        Command::new("wtype")
            .arg(text)
            .status()
            .map_err(|e| PlatformError::Paste(format!("wtype failed (is it installed?): {}", e)))?;
        Ok(())
    }

    /// Paste using xdotool (X11)
    fn paste_x11(&self, text: &str) -> Result<(), PlatformError> {
        // First copy to clipboard, then paste with Ctrl+V
        self.copy_to_clipboard(text)?;

        Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .status()
            .map_err(|e| {
                PlatformError::Paste(format!("xdotool failed (is it installed?): {}", e))
            })?;
        Ok(())
    }

    /// Output to stdout (TTY mode)
    fn paste_tty(&self, text: &str) -> Result<(), PlatformError> {
        // In TTY mode, we just print to stdout
        // The user can pipe this or use it as needed
        print!("{}", text);
        Ok(())
    }
}

impl Default for LinuxPlatform {
    /// Creates a LinuxPlatform with default settings.
    ///
    /// # Panics
    /// This implementation cannot currently panic as `LinuxPlatform::new()` only
    /// performs infallible operations. However, prefer using `LinuxPlatform::new()`
    /// directly for explicit error handling.
    fn default() -> Self {
        // LinuxPlatform::new() currently cannot fail, but we handle the Result
        // for forward compatibility if initialization becomes fallible.
        Self::new().unwrap_or_else(|e| {
            // Log the error and create a minimal fallback
            eprintln!(
                "Warning: LinuxPlatform initialization failed: {}. Using fallback.",
                e
            );
            Self {
                display_server: super::DisplayServer::Tty,
                clipboard: std::sync::Mutex::new(None),
            }
        })
    }
}

impl HotkeyHandler for LinuxPlatform {
    fn start(&mut self, _key: &str) -> Result<(), PlatformError> {
        // TODO: Implement using rdev
        // For TTY mode, use evdev
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn poll(&mut self) -> Option<HotkeyEvent> {
        // TODO: Implement
        None
    }
}

impl TextOutput for LinuxPlatform {
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
        match self.display_server {
            DisplayServer::Wayland => self.paste_wayland(text),
            DisplayServer::X11 => self.paste_x11(text),
            DisplayServer::Tty => self.paste_tty(text),
            _ => Err(PlatformError::NotSupported("Unknown display server".into())),
        }
    }
}

impl Notifier for LinuxPlatform {
    fn notify(&self, title: &str, body: &str) -> Result<(), PlatformError> {
        if self.display_server == DisplayServer::Tty {
            // No notifications in TTY mode
            return Ok(());
        }

        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .appname("OpenHush")
            .show()
            .map_err(|e| PlatformError::Notification(e.to_string()))?;

        Ok(())
    }
}

impl AudioFeedback for LinuxPlatform {
    fn play_start_sound(&self) -> Result<(), PlatformError> {
        // TODO: Implement using rodio
        // For now, use system bell
        print!("\x07");
        Ok(())
    }

    fn play_stop_sound(&self) -> Result<(), PlatformError> {
        // TODO: Implement using rodio with different tone
        print!("\x07");
        Ok(())
    }
}

impl Platform for LinuxPlatform {
    fn display_server(&self) -> &str {
        match self.display_server {
            DisplayServer::X11 => "X11",
            DisplayServer::Wayland => "Wayland",
            DisplayServer::Tty => "TTY",
            _ => "Unknown",
        }
    }

    fn is_tty(&self) -> bool {
        self.display_server == DisplayServer::Tty
    }
}

/// Linux system tray implementation.
///
/// Note: The actual tray is managed by crate::tray::TrayManager in the daemon.
/// This struct provides a simplified interface for the platform abstraction layer.
pub struct LinuxSystemTray {
    status: TrayStatus,
}

impl SystemTray for LinuxSystemTray {
    fn new() -> Result<Self, PlatformError> {
        // The actual tray initialization is handled by TrayManager in the daemon
        // This is just a placeholder for the platform abstraction
        Ok(Self {
            status: TrayStatus::Idle,
        })
    }

    fn set_status(&mut self, status: TrayStatus) {
        self.status = status;
        // Actual status updates are handled by TrayManager
    }

    fn poll_event(&mut self) -> Option<TrayMenuEvent> {
        // Events are handled by TrayManager in the daemon
        None
    }

    fn is_supported() -> bool {
        // Check for D-Bus session bus (required for ksni)
        std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok()
    }
}
