//! Input handling: hotkey detection and audio capture.

pub mod audio;
pub mod hotkey;
pub mod ring_buffer;

pub use audio::{AudioBuffer, AudioRecorder, AudioRecorderError};
pub use hotkey::{HotkeyEvent, HotkeyListener, HotkeyListenerError};
pub use ring_buffer::AudioMark;
#[allow(unused_imports)]
pub use ring_buffer::AudioRingBuffer;
