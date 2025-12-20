//! Windows platform implementation
//!
//! Uses SendInput for text input and Win32 APIs for clipboard/notifications.

use super::{
    AudioFeedback, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError, TextOutput,
};

pub struct WindowsPlatform {
    // TODO: Add Windows-specific state
}

impl WindowsPlatform {
    pub fn new() -> Result<Self, PlatformError> {
        Ok(Self {})
    }
}

impl Default for WindowsPlatform {
    /// Creates a WindowsPlatform with default settings.
    ///
    /// This cannot panic as `WindowsPlatform::new()` always succeeds.
    fn default() -> Self {
        // WindowsPlatform::new() is infallible (returns empty struct)
        Self::new().expect("WindowsPlatform::new is infallible")
    }
}

impl HotkeyHandler for WindowsPlatform {
    fn start(&mut self, _key: &str) -> Result<(), PlatformError> {
        // TODO: Implement using rdev or RegisterHotKey
        Err(PlatformError::NotSupported(
            "Windows hotkeys not yet implemented".into(),
        ))
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn poll(&mut self) -> Option<HotkeyEvent> {
        None
    }
}

impl TextOutput for WindowsPlatform {
    fn copy_to_clipboard(&self, _text: &str) -> Result<(), PlatformError> {
        // TODO: Implement using arboard
        Err(PlatformError::NotSupported(
            "Windows clipboard not yet implemented".into(),
        ))
    }

    fn paste_text(&self, _text: &str) -> Result<(), PlatformError> {
        // TODO: Implement using enigo (SendInput)
        Err(PlatformError::NotSupported(
            "Windows paste not yet implemented".into(),
        ))
    }
}

impl Notifier for WindowsPlatform {
    fn notify(&self, _title: &str, _body: &str) -> Result<(), PlatformError> {
        // TODO: Implement using notify-rust or winrt-toast
        Err(PlatformError::NotSupported(
            "Windows notifications not yet implemented".into(),
        ))
    }
}

impl AudioFeedback for WindowsPlatform {
    fn play_start_sound(&self) -> Result<(), PlatformError> {
        // TODO: Implement using rodio
        Ok(())
    }

    fn play_stop_sound(&self) -> Result<(), PlatformError> {
        // TODO: Implement using rodio
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
