//! Transcription engine using Whisper.

pub mod validation;
pub mod whisper;

#[allow(unused_imports)]
pub use validation::{validate_audio, AudioValidationError, AudioValidationInfo};
#[allow(unused_imports)]
pub use whisper::{BenchmarkResult, WhisperEngine, WhisperError};
