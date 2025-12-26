//! macOS platform implementation
//!
//! Uses arboard for clipboard, enigo for text input, and notify-rust for notifications.
//! These crates provide cross-platform APIs that work on macOS.
//!
//! Note: Accessibility permissions are required for keyboard simulation and global hotkeys.
//! The user will be prompted by macOS when first using the app.

use super::{
    AudioFeedback, HotkeyEvent, HotkeyHandler, Notifier, Platform, PlatformError, SystemTray,
    TextOutput, TrayMenuEvent, TrayStatus,
};
use arboard::Clipboard;
use macos_accessibility_client::accessibility;
use std::sync::Mutex;
use tracing::{info, warn};

/// Result of accessibility permission check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityStatus {
    /// Accessibility is enabled
    Granted,
    /// Accessibility is not enabled (user needs to grant permission)
    Denied,
    /// Could not determine accessibility status
    Unknown,
}

/// Check if accessibility permissions are granted.
///
/// OpenHush requires accessibility permissions on macOS for:
/// - Global hotkey detection (listening for key events)
/// - Simulating keyboard input (pasting transcribed text)
///
/// Returns `AccessibilityStatus::Granted` if permissions are enabled.
pub fn check_accessibility() -> AccessibilityStatus {
    if accessibility::application_is_trusted() {
        AccessibilityStatus::Granted
    } else {
        AccessibilityStatus::Denied
    }
}

/// Check accessibility and prompt user if not granted.
///
/// If accessibility is not enabled, this will trigger the macOS system prompt
/// asking the user to grant accessibility permissions in System Preferences.
///
/// Returns true if accessibility is already granted, false otherwise.
pub fn check_accessibility_with_prompt() -> bool {
    if accessibility::application_is_trusted() {
        true
    } else {
        // Trigger the system prompt
        accessibility::application_is_trusted_with_prompt();
        false
    }
}

/// Open System Preferences to the Accessibility pane.
///
/// This is useful when you want to direct the user to the settings
/// without triggering the system prompt.
pub fn open_accessibility_preferences() -> Result<(), PlatformError> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map_err(|e| PlatformError::Other(format!("Failed to open System Preferences: {}", e)))?;
    Ok(())
}

/// Print accessibility permission instructions to stderr.
pub fn print_accessibility_instructions() {
    eprintln!();
    eprintln!("=======================================================");
    eprintln!("  ACCESSIBILITY PERMISSION REQUIRED");
    eprintln!("=======================================================");
    eprintln!();
    eprintln!("OpenHush needs Accessibility permissions to:");
    eprintln!("  - Detect global hotkeys (trigger recording)");
    eprintln!("  - Paste transcribed text into applications");
    eprintln!();
    eprintln!("To grant permission:");
    eprintln!("  1. Open System Preferences > Privacy & Security > Accessibility");
    eprintln!("  2. Click the lock icon to make changes");
    eprintln!("  3. Add OpenHush to the allowed applications");
    eprintln!("  4. Restart OpenHush");
    eprintln!();
    eprintln!("Or run: open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'");
    eprintln!("=======================================================");
    eprintln!();
}

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

    /// Check and report accessibility permissions.
    ///
    /// Returns Ok(()) if accessibility is granted, or an error with instructions otherwise.
    pub fn check_accessibility_permissions(&self) -> Result<(), PlatformError> {
        match check_accessibility() {
            AccessibilityStatus::Granted => {
                info!("Accessibility permissions granted");
                Ok(())
            }
            AccessibilityStatus::Denied => {
                warn!("Accessibility permissions not granted");
                print_accessibility_instructions();

                // Trigger the system prompt
                if !check_accessibility_with_prompt() {
                    return Err(PlatformError::Accessibility(
                        "Accessibility permission required. Please grant permission in System Preferences.".into()
                    ));
                }
                Ok(())
            }
            AccessibilityStatus::Unknown => {
                warn!("Could not determine accessibility status");
                Ok(()) // Continue anyway, let macOS handle it
            }
        }
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
        self.paste_via_enigo(text)
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // AccessibilityStatus Tests
    // ===================

    #[test]
    fn test_accessibility_status_equality() {
        assert_eq!(AccessibilityStatus::Granted, AccessibilityStatus::Granted);
        assert_eq!(AccessibilityStatus::Denied, AccessibilityStatus::Denied);
        assert_eq!(AccessibilityStatus::Unknown, AccessibilityStatus::Unknown);
        assert_ne!(AccessibilityStatus::Granted, AccessibilityStatus::Denied);
        assert_ne!(AccessibilityStatus::Denied, AccessibilityStatus::Unknown);
    }

    #[test]
    fn test_accessibility_status_debug() {
        assert_eq!(format!("{:?}", AccessibilityStatus::Granted), "Granted");
        assert_eq!(format!("{:?}", AccessibilityStatus::Denied), "Denied");
        assert_eq!(format!("{:?}", AccessibilityStatus::Unknown), "Unknown");
    }

    #[test]
    fn test_accessibility_status_clone() {
        let status = AccessibilityStatus::Granted;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    // ===================
    // MacOSSystemTray Tests
    // ===================

    #[test]
    fn test_macos_system_tray_is_supported() {
        assert!(MacOSSystemTray::is_supported());
    }

    #[test]
    fn test_macos_system_tray_new() {
        let tray = MacOSSystemTray::new();
        assert!(tray.is_ok());
    }

    #[test]
    fn test_macos_system_tray_set_status() {
        let mut tray = MacOSSystemTray::new().unwrap();
        tray.set_status(TrayStatus::Recording);
        // Status is stored internally
        assert_eq!(tray.status, TrayStatus::Recording);
    }

    #[test]
    fn test_macos_system_tray_poll_event() {
        let mut tray = MacOSSystemTray::new().unwrap();
        // poll_event returns None (events handled by tray manager)
        assert!(tray.poll_event().is_none());
    }

    // ===================
    // MacOSPlatform Tests
    // ===================

    #[test]
    fn test_macos_platform_display_server() {
        let platform = MacOSPlatform::new().unwrap();
        assert_eq!(platform.display_server(), "macOS");
    }

    #[test]
    fn test_macos_platform_is_tty() {
        let platform = MacOSPlatform::new().unwrap();
        assert!(!platform.is_tty());
    }

    #[test]
    fn test_macos_platform_default() {
        let platform = MacOSPlatform::default();
        assert_eq!(platform.display_server(), "macOS");
    }
}
