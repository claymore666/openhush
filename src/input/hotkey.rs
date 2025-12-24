//! Global hotkey detection using rdev.
//!
//! Listens for keyboard events and emits HotkeyEvents when the configured
//! hotkey is pressed or released.

use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum HotkeyListenerError {
    #[error("Failed to start hotkey listener: {0}")]
    #[allow(dead_code)]
    StartFailed(String),

    #[error("Invalid hotkey: {0}")]
    InvalidHotkey(String),

    #[error("Listener stopped unexpectedly")]
    #[allow(dead_code)]
    ListenerStopped,
}

/// Events emitted by the hotkey listener
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// Hotkey was pressed (start recording)
    Pressed,
    /// Hotkey was released (stop recording)
    Released,
}

/// Global hotkey listener
pub struct HotkeyListener {
    key: Key,
    running: Arc<AtomicBool>,
    event_tx: mpsc::Sender<HotkeyEvent>,
}

impl HotkeyListener {
    /// Create a new hotkey listener for the given key string
    pub fn new(key_str: &str) -> Result<(Self, mpsc::Receiver<HotkeyEvent>), HotkeyListenerError> {
        let key = parse_key(key_str)?;
        let (event_tx, event_rx) = mpsc::channel(32);
        let running = Arc::new(AtomicBool::new(false));

        Ok((
            Self {
                key,
                running,
                event_tx,
            },
            event_rx,
        ))
    }

    /// Start listening for hotkey events
    ///
    /// This spawns a background thread that listens for keyboard events.
    /// Returns immediately; use the receiver to get events.
    pub fn start(&self) -> Result<(), HotkeyListenerError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        self.running.store(true, Ordering::SeqCst);

        let key = self.key;
        let running = self.running.clone();
        let event_tx = self.event_tx.clone();

        // Track key state to avoid duplicate events
        let key_pressed = Arc::new(AtomicBool::new(false));
        let key_pressed_clone = key_pressed.clone();

        thread::spawn(move || {
            info!("Hotkey listener started for {:?}", key);

            let callback = move |event: Event| {
                match event.event_type {
                    EventType::KeyPress(pressed_key) if pressed_key == key => {
                        // Only emit if not already pressed (avoid key repeat)
                        if !key_pressed_clone.swap(true, Ordering::SeqCst) {
                            debug!("Hotkey pressed: {:?}", key);
                            if let Err(e) = event_tx.blocking_send(HotkeyEvent::Pressed) {
                                error!("Failed to send hotkey event: {}", e);
                            }
                        }
                    }
                    EventType::KeyRelease(released_key) if released_key == key => {
                        if key_pressed_clone.swap(false, Ordering::SeqCst) {
                            debug!("Hotkey released: {:?}", key);
                            if let Err(e) = event_tx.blocking_send(HotkeyEvent::Released) {
                                error!("Failed to send hotkey event: {}", e);
                            }
                        }
                    }
                    _ => {}
                }
            };

            if let Err(e) = listen(callback) {
                error!("Hotkey listener error: {:?}", e);
                running.store(false, Ordering::SeqCst);
            }
        });

        Ok(())
    }

    /// Stop listening for hotkey events
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        // Note: rdev doesn't have a clean way to stop the listener
        // The thread will continue until the process exits
        warn!("Hotkey listener stop requested (may not fully stop until process exit)");
    }

    /// Check if the listener is running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Parse a key string into an rdev Key
///
/// Supports formats like:
/// - "ControlRight", "ControlLeft", "ctrl_r", "ctrl_l"
/// - "AltRight", "AltLeft", "alt_r", "alt_l"
/// - "ShiftRight", "ShiftLeft", "shift_r", "shift_l"
/// - "F1" through "F12"
/// - "Space", "Escape", "Tab", etc.
pub fn parse_key(key_str: &str) -> Result<Key, HotkeyListenerError> {
    let normalized = key_str.to_lowercase().replace(['_', '-'], "");

    match normalized.as_str() {
        // Control keys
        "controlright" | "ctrlr" | "ctrlright" | "rctrl" => Ok(Key::ControlRight),
        "controlleft" | "ctrll" | "ctrlleft" | "lctrl" | "ctrl" => Ok(Key::ControlLeft),

        // Alt keys
        "altright" | "altr" | "ralt" | "altgr" => Ok(Key::AltGr), // Right Alt / AltGr
        "altleft" | "altl" | "lalt" | "alt" => Ok(Key::Alt),

        // Shift keys
        "shiftright" | "shiftr" | "rshift" => Ok(Key::ShiftRight),
        "shiftleft" | "shiftl" | "lshift" | "shift" => Ok(Key::ShiftLeft),

        // Meta/Super/Windows keys
        "metaleft" | "superleft" | "winleft" | "lsuper" | "lmeta" | "lwin" => Ok(Key::MetaLeft),
        "metaright" | "superright" | "winright" | "rsuper" | "rmeta" | "rwin" => Ok(Key::MetaRight),

        // Function keys
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),

        // Special keys
        "space" => Ok(Key::Space),
        "escape" | "esc" => Ok(Key::Escape),
        "tab" => Ok(Key::Tab),
        "capslock" | "caps" => Ok(Key::CapsLock),
        "backspace" | "back" => Ok(Key::Backspace),
        "enter" | "return" => Ok(Key::Return),

        // Fallback: try to parse as rdev Key directly
        _ => Err(HotkeyListenerError::InvalidHotkey(format!(
            "Unknown key: '{}'. Valid examples: ControlRight, ctrl_r, F12, Space",
            key_str
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // Control Key Tests
    // ===================

    #[test]
    fn test_parse_key_control_right() {
        assert_eq!(parse_key("ControlRight").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("ctrl_r").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("CTRL_R").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("ctrlright").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("rctrl").unwrap(), Key::ControlRight);
    }

    #[test]
    fn test_parse_key_control_left() {
        assert_eq!(parse_key("ControlLeft").unwrap(), Key::ControlLeft);
        assert_eq!(parse_key("ctrl_l").unwrap(), Key::ControlLeft);
        assert_eq!(parse_key("ctrl").unwrap(), Key::ControlLeft);
        assert_eq!(parse_key("lctrl").unwrap(), Key::ControlLeft);
    }

    // ===================
    // Alt Key Tests
    // ===================

    #[test]
    fn test_parse_key_alt_left() {
        assert_eq!(parse_key("AltLeft").unwrap(), Key::Alt);
        assert_eq!(parse_key("alt_l").unwrap(), Key::Alt);
        assert_eq!(parse_key("alt").unwrap(), Key::Alt);
        assert_eq!(parse_key("lalt").unwrap(), Key::Alt);
    }

    #[test]
    fn test_parse_key_alt_right() {
        assert_eq!(parse_key("AltRight").unwrap(), Key::AltGr);
        assert_eq!(parse_key("alt_r").unwrap(), Key::AltGr);
        assert_eq!(parse_key("altgr").unwrap(), Key::AltGr);
        assert_eq!(parse_key("ralt").unwrap(), Key::AltGr);
    }

    // ===================
    // Shift Key Tests
    // ===================

    #[test]
    fn test_parse_key_shift_left() {
        assert_eq!(parse_key("ShiftLeft").unwrap(), Key::ShiftLeft);
        assert_eq!(parse_key("shift_l").unwrap(), Key::ShiftLeft);
        assert_eq!(parse_key("shift").unwrap(), Key::ShiftLeft);
        assert_eq!(parse_key("lshift").unwrap(), Key::ShiftLeft);
    }

    #[test]
    fn test_parse_key_shift_right() {
        assert_eq!(parse_key("ShiftRight").unwrap(), Key::ShiftRight);
        assert_eq!(parse_key("shift_r").unwrap(), Key::ShiftRight);
        assert_eq!(parse_key("rshift").unwrap(), Key::ShiftRight);
    }

    // ===================
    // Meta/Super Key Tests
    // ===================

    #[test]
    fn test_parse_key_meta_left() {
        assert_eq!(parse_key("MetaLeft").unwrap(), Key::MetaLeft);
        assert_eq!(parse_key("superleft").unwrap(), Key::MetaLeft);
        assert_eq!(parse_key("winleft").unwrap(), Key::MetaLeft);
        assert_eq!(parse_key("lsuper").unwrap(), Key::MetaLeft);
        assert_eq!(parse_key("lmeta").unwrap(), Key::MetaLeft);
        assert_eq!(parse_key("lwin").unwrap(), Key::MetaLeft);
    }

    #[test]
    fn test_parse_key_meta_right() {
        assert_eq!(parse_key("MetaRight").unwrap(), Key::MetaRight);
        assert_eq!(parse_key("superright").unwrap(), Key::MetaRight);
        assert_eq!(parse_key("winright").unwrap(), Key::MetaRight);
        assert_eq!(parse_key("rsuper").unwrap(), Key::MetaRight);
    }

    // ===================
    // Function Key Tests
    // ===================

    #[test]
    fn test_parse_key_function_keys() {
        assert_eq!(parse_key("F1").unwrap(), Key::F1);
        assert_eq!(parse_key("f12").unwrap(), Key::F12);
    }

    #[test]
    fn test_parse_key_all_function_keys() {
        assert_eq!(parse_key("f1").unwrap(), Key::F1);
        assert_eq!(parse_key("f2").unwrap(), Key::F2);
        assert_eq!(parse_key("f3").unwrap(), Key::F3);
        assert_eq!(parse_key("f4").unwrap(), Key::F4);
        assert_eq!(parse_key("f5").unwrap(), Key::F5);
        assert_eq!(parse_key("f6").unwrap(), Key::F6);
        assert_eq!(parse_key("f7").unwrap(), Key::F7);
        assert_eq!(parse_key("f8").unwrap(), Key::F8);
        assert_eq!(parse_key("f9").unwrap(), Key::F9);
        assert_eq!(parse_key("f10").unwrap(), Key::F10);
        assert_eq!(parse_key("f11").unwrap(), Key::F11);
        assert_eq!(parse_key("f12").unwrap(), Key::F12);
    }

    // ===================
    // Special Key Tests
    // ===================

    #[test]
    fn test_parse_key_special() {
        assert_eq!(parse_key("Space").unwrap(), Key::Space);
        assert_eq!(parse_key("escape").unwrap(), Key::Escape);
        assert_eq!(parse_key("esc").unwrap(), Key::Escape);
    }

    #[test]
    fn test_parse_key_all_special() {
        assert_eq!(parse_key("space").unwrap(), Key::Space);
        assert_eq!(parse_key("tab").unwrap(), Key::Tab);
        assert_eq!(parse_key("capslock").unwrap(), Key::CapsLock);
        assert_eq!(parse_key("caps").unwrap(), Key::CapsLock);
        assert_eq!(parse_key("backspace").unwrap(), Key::Backspace);
        assert_eq!(parse_key("back").unwrap(), Key::Backspace);
        assert_eq!(parse_key("enter").unwrap(), Key::Return);
        assert_eq!(parse_key("return").unwrap(), Key::Return);
    }

    // ===================
    // Case Insensitivity Tests
    // ===================

    #[test]
    fn test_parse_key_case_insensitive() {
        assert_eq!(parse_key("SPACE").unwrap(), Key::Space);
        assert_eq!(parse_key("Space").unwrap(), Key::Space);
        assert_eq!(parse_key("space").unwrap(), Key::Space);
        assert_eq!(parse_key("CONTROLRIGHT").unwrap(), Key::ControlRight);
    }

    // ===================
    // Normalization Tests
    // ===================

    #[test]
    fn test_parse_key_with_underscores() {
        assert_eq!(parse_key("control_right").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("ctrl_r").unwrap(), Key::ControlRight);
    }

    #[test]
    fn test_parse_key_with_dashes() {
        assert_eq!(parse_key("control-right").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("ctrl-r").unwrap(), Key::ControlRight);
    }

    // ===================
    // Invalid Key Tests
    // ===================

    #[test]
    fn test_parse_key_invalid() {
        assert!(parse_key("invalid_key").is_err());
        assert!(parse_key("").is_err());
    }

    #[test]
    fn test_parse_key_invalid_error_message() {
        let result = parse_key("invalid_key");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Unknown key"));
        assert!(err.to_string().contains("invalid_key"));
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
    // Error Tests
    // ===================

    #[test]
    fn test_hotkey_listener_error_display() {
        let err = HotkeyListenerError::InvalidHotkey("test".to_string());
        assert!(err.to_string().contains("Invalid hotkey"));
        assert!(err.to_string().contains("test"));

        let err = HotkeyListenerError::StartFailed("failed".to_string());
        assert!(err.to_string().contains("Failed to start"));

        let err = HotkeyListenerError::ListenerStopped;
        assert!(err.to_string().contains("stopped unexpectedly"));
    }

    // ===================
    // HotkeyListener Creation Tests
    // ===================

    #[test]
    fn test_hotkey_listener_new_valid() {
        let result = HotkeyListener::new("ControlRight");
        assert!(result.is_ok());
    }

    #[test]
    fn test_hotkey_listener_new_invalid() {
        let result = HotkeyListener::new("invalid_key_xyz");
        assert!(result.is_err());
    }
}
