//! Transcription engine using Whisper.

pub mod whisper;

#[allow(unused_imports)]
pub use whisper::{BenchmarkResult, WhisperEngine, WhisperError};
