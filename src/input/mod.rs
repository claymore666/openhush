//! Input handling: hotkey detection and audio capture.

pub mod audio;
pub mod hotkey;

pub use audio::{AudioBuffer, AudioRecorder, AudioRecorderError};
pub use hotkey::{HotkeyEvent, HotkeyListener, HotkeyListenerError};
