//! Input handling: hotkey detection and audio capture.

pub mod audio;
pub mod hotkey;
pub mod ring_buffer;
#[cfg(target_os = "linux")]
pub mod system_audio;
// TODO: Blocked by candle-core f16 bug: https://github.com/huggingface/candle/issues/2805
// pub mod wake_word;

pub use audio::{load_wav_file, AudioBuffer, AudioRecorder, AudioRecorderError};
pub use hotkey::{HotkeyEvent, HotkeyListener, HotkeyListenerError};
pub use ring_buffer::AudioMark;
#[allow(unused_imports)]
pub use ring_buffer::AudioRingBuffer;
#[allow(unused_imports)]
#[cfg(target_os = "linux")]
pub use system_audio::{AudioSource, SourceInfo, SystemAudioCapture, SystemAudioError};
