//! macOS platform implementation
//!
//! Uses CGEvent for text input and native APIs for clipboard/notifications.

use super::{
    AudioFeedback, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError, TextOutput,
};

pub struct MacOSPlatform {
    // TODO: Add macOS-specific state
}

impl MacOSPlatform {
    pub fn new() -> Result<Self, PlatformError> {
        Ok(Self {})
    }
}

impl Default for MacOSPlatform {
    fn default() -> Self {
        Self::new().expect("Failed to create MacOSPlatform")
    }
}

impl HotkeyHandler for MacOSPlatform {
    fn start(&mut self, _key: &str) -> Result<(), PlatformError> {
        // TODO: Implement using rdev or CGEvent
        Err(PlatformError::NotSupported("macOS hotkeys not yet implemented".into()))
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
        Err(PlatformError::NotSupported("macOS clipboard not yet implemented".into()))
    }

    fn paste_text(&self, _text: &str) -> Result<(), PlatformError> {
        // TODO: Implement using enigo (CGEvent)
        Err(PlatformError::NotSupported("macOS paste not yet implemented".into()))
    }
}

impl Notifier for MacOSPlatform {
    fn notify(&self, _title: &str, _body: &str) -> Result<(), PlatformError> {
        // TODO: Implement using notify-rust or osascript
        Err(PlatformError::NotSupported("macOS notifications not yet implemented".into()))
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
