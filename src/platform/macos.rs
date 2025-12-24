//! macOS platform implementation
//!
//! Uses CGEvent for text input and native APIs for clipboard/notifications.
//!
//! NOTE: Full implementation pending. Currently returns NotSupported for most operations.
//! The enigo crate's CGEventSource is not Send+Sync on macOS.

use super::{
    AudioFeedback, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError, SystemTray,
    TextOutput, TrayMenuEvent, TrayStatus,
};

pub struct MacOSPlatform {
    // TODO: Add macOS-specific state when implemented
}

impl MacOSPlatform {
    pub fn new() -> Result<Self, PlatformError> {
        Ok(Self {})
    }
}

impl Default for MacOSPlatform {
    /// Creates a MacOSPlatform with default settings.
    ///
    /// This cannot panic as `MacOSPlatform::new()` always succeeds.
    fn default() -> Self {
        // MacOSPlatform::new() is infallible (returns empty struct)
        Self::new().expect("MacOSPlatform::new is infallible")
    }
}

impl HotkeyHandler for MacOSPlatform {
    fn start(&mut self, _key: &str) -> Result<(), PlatformError> {
        // TODO: Implement using rdev or CGEvent
        Err(PlatformError::NotSupported(
            "macOS hotkeys not yet implemented".into(),
        ))
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn poll(&mut self) -> Option<HotkeyEvent> {
        None
    }
}

impl TextOutput for MacOSPlatform {
    fn copy_to_clipboard(&self, _text: &str) -> Result<(), PlatformError> {
        // TODO: Implement using arboard
        Err(PlatformError::NotSupported(
            "macOS clipboard not yet implemented".into(),
        ))
    }

    fn paste_text(&self, _text: &str) -> Result<(), PlatformError> {
        // TODO: Implement using enigo or CGEvent directly
        // Note: enigo's CGEventSource is not Send+Sync, needs alternative approach
        Err(PlatformError::NotSupported(
            "macOS paste not yet implemented".into(),
        ))
    }
}

impl Notifier for MacOSPlatform {
    fn notify(&self, _title: &str, _body: &str) -> Result<(), PlatformError> {
        // TODO: Implement using notify-rust or osascript
        Err(PlatformError::NotSupported(
            "macOS notifications not yet implemented".into(),
        ))
    }
}

impl AudioFeedback for MacOSPlatform {
    fn play_start_sound(&self) -> Result<(), PlatformError> {
        // TODO: Implement using rodio
        Ok(())
    }

    fn play_stop_sound(&self) -> Result<(), PlatformError> {
        // TODO: Implement using rodio
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

/// macOS system tray implementation (stub).
///
/// TODO: Implement using NSStatusItem or tray-icon crate.
pub struct MacOSSystemTray {
    status: TrayStatus,
}

impl SystemTray for MacOSSystemTray {
    fn new() -> Result<Self, PlatformError> {
        // TODO: Implement macOS tray
        Ok(Self {
            status: TrayStatus::Idle,
        })
    }

    fn set_status(&mut self, status: TrayStatus) {
        self.status = status;
        // TODO: Update tray icon/tooltip
    }

    fn poll_event(&mut self) -> Option<TrayMenuEvent> {
        // TODO: Implement event polling
        None
    }

    fn is_supported() -> bool {
        // macOS always has menu bar support
        true
    }
}
