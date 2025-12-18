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
        "altright" | "altr" | "ralt" => Ok(Key::Alt), // rdev uses Alt for AltRight
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

    #[test]
    fn test_parse_key_control_right() {
        assert_eq!(parse_key("ControlRight").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("ctrl_r").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("CTRL_R").unwrap(), Key::ControlRight);
        assert_eq!(parse_key("ctrlright").unwrap(), Key::ControlRight);
    }

    #[test]
    fn test_parse_key_control_left() {
        assert_eq!(parse_key("ControlLeft").unwrap(), Key::ControlLeft);
        assert_eq!(parse_key("ctrl_l").unwrap(), Key::ControlLeft);
        assert_eq!(parse_key("ctrl").unwrap(), Key::ControlLeft);
    }

    #[test]
    fn test_parse_key_function_keys() {
        assert_eq!(parse_key("F1").unwrap(), Key::F1);
        assert_eq!(parse_key("f12").unwrap(), Key::F12);
    }

    #[test]
    fn test_parse_key_special() {
        assert_eq!(parse_key("Space").unwrap(), Key::Space);
        assert_eq!(parse_key("escape").unwrap(), Key::Escape);
        assert_eq!(parse_key("esc").unwrap(), Key::Escape);
    }

    #[test]
    fn test_parse_key_invalid() {
        assert!(parse_key("invalid_key").is_err());
        assert!(parse_key("").is_err());
    }
}
