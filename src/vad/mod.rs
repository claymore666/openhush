//! Voice Activity Detection (VAD) module.
//!
//! Provides real-time speech detection using the Silero VAD model
//! with ONNX inference via ort/silero-vad-rust.

pub mod silero;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// VAD-related errors.
#[derive(Error, Debug)]
pub enum VadError {
    #[error("Failed to load VAD model: {0}")]
    ModelLoad(String),

    #[error("Inference error: {0}")]
    Inference(String),
}

/// Result of VAD analysis for an audio chunk.
#[derive(Debug, Clone)]
pub struct VadResult {
    /// Speech probability (0.0 to 1.0)
    pub probability: f32,
    /// Whether speech was detected (probability >= threshold)
    pub is_speech: bool,
}

/// Voice Activity Detection engine trait.
///
/// Implementations must be stateful to handle LSTM hidden states
/// between consecutive audio chunks.
#[allow(dead_code)]
pub trait VadEngine: Send {
    /// Process an audio chunk and return VAD result.
    ///
    /// # Arguments
    /// * `samples` - Audio samples at 16kHz, mono, f32 [-1.0, 1.0]
    ///
    /// # Returns
    /// VAD result with speech probability
    fn process(&mut self, samples: &[f32]) -> Result<VadResult, VadError>;

    /// Reset internal state (LSTM hidden states).
    ///
    /// Call this when starting a new recording session.
    fn reset(&mut self);

    /// Get the expected chunk size in samples.
    fn chunk_size(&self) -> usize;

    /// Get the sample rate this VAD expects.
    fn sample_rate(&self) -> u32;
}

/// VAD configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Enable VAD for continuous dictation mode
    #[serde(default)]
    pub enabled: bool,
    /// Speech probability threshold (0.0 to 1.0)
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    /// Minimum silence duration before ending speech (ms)
    #[serde(default = "default_min_silence_ms")]
    pub min_silence_ms: u32,
    /// Minimum speech duration to be considered valid (ms)
    #[serde(default = "default_min_speech_ms")]
    pub min_speech_ms: u32,
    /// Padding to add before/after speech segments (ms)
    #[serde(default = "default_speech_pad_ms")]
    pub speech_pad_ms: u32,
}

fn default_threshold() -> f32 {
    0.5
}

fn default_min_silence_ms() -> u32 {
    700
}

fn default_min_speech_ms() -> u32 {
    250
}

fn default_speech_pad_ms() -> u32 {
    30
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: default_threshold(),
            min_silence_ms: default_min_silence_ms(),
            min_speech_ms: default_min_speech_ms(),
            speech_pad_ms: default_speech_pad_ms(),
        }
    }
}

/// Speech segment detected by VAD.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SpeechSegment {
    /// Start position in samples
    pub start: usize,
    /// End position in samples
    pub end: usize,
    /// Average speech probability
    pub avg_probability: f32,
}

/// Streaming VAD state tracker.
///
/// Tracks speech/silence transitions and emits speech segments.
#[derive(Debug)]
pub struct VadState {
    config: VadConfig,
    sample_rate: u32,
    /// Current speech probability history
    probabilities: Vec<f32>,
    /// Whether currently in speech
    in_speech: bool,
    /// Sample position where current speech started
    speech_start: Option<usize>,
    /// Samples of silence since last speech
    silence_samples: usize,
    /// Total samples processed
    total_samples: usize,
}

impl VadState {
    /// Create a new VAD state tracker.
    pub fn new(config: VadConfig, sample_rate: u32) -> Self {
        Self {
            config,
            sample_rate,
            probabilities: Vec::new(),
            in_speech: false,
            speech_start: None,
            silence_samples: 0,
            total_samples: 0,
        }
    }

    /// Update state with a new VAD result.
    ///
    /// # Arguments
    /// * `result` - VAD result for the current chunk
    /// * `chunk_samples` - Number of samples in the chunk
    ///
    /// # Returns
    /// Completed speech segment if speech just ended, None otherwise
    pub fn update(&mut self, result: &VadResult, chunk_samples: usize) -> Option<SpeechSegment> {
        self.probabilities.push(result.probability);
        let prev_total = self.total_samples;
        self.total_samples += chunk_samples;

        let min_silence_samples =
            (self.config.min_silence_ms as f32 / 1000.0 * self.sample_rate as f32) as usize;
        let min_speech_samples =
            (self.config.min_speech_ms as f32 / 1000.0 * self.sample_rate as f32) as usize;

        if result.is_speech {
            self.silence_samples = 0;

            if !self.in_speech {
                // Speech just started
                self.in_speech = true;
                self.speech_start = Some(prev_total);
                tracing::debug!(
                    "Speech started at sample {} (prob: {:.2})",
                    prev_total,
                    result.probability
                );
            }
            None
        } else {
            self.silence_samples += chunk_samples;

            if self.in_speech && self.silence_samples >= min_silence_samples {
                // Speech ended
                self.in_speech = false;
                let start = self.speech_start.take().unwrap_or(0);
                let end = prev_total; // End at start of silence

                // Check minimum duration
                if end - start >= min_speech_samples {
                    let avg_prob = if !self.probabilities.is_empty() {
                        self.probabilities.iter().sum::<f32>() / self.probabilities.len() as f32
                    } else {
                        0.0
                    };
                    self.probabilities.clear();

                    tracing::debug!(
                        "Speech ended: {} - {} ({} samples, avg prob: {:.2})",
                        start,
                        end,
                        end - start,
                        avg_prob
                    );

                    return Some(SpeechSegment {
                        start,
                        end,
                        avg_probability: avg_prob,
                    });
                } else {
                    tracing::debug!(
                        "Speech too short: {} samples (min: {})",
                        end - start,
                        min_speech_samples
                    );
                    self.probabilities.clear();
                }
            }
            None
        }
    }

    /// Check if currently detecting speech.
    #[allow(dead_code)]
    pub fn is_speech(&self) -> bool {
        self.in_speech
    }

    /// Get the current speech start position, if in speech.
    #[allow(dead_code)]
    pub fn speech_start(&self) -> Option<usize> {
        self.speech_start
    }

    /// Reset state for a new recording.
    pub fn reset(&mut self) {
        self.probabilities.clear();
        self.in_speech = false;
        self.speech_start = None;
        self.silence_samples = 0;
        self.total_samples = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_state_speech_detection() {
        let config = VadConfig {
            threshold: 0.5,
            min_silence_ms: 100,
            min_speech_ms: 50,
            ..Default::default()
        };
        let mut state = VadState::new(config, 16000);

        // Simulate speech detection
        let speech = VadResult {
            probability: 0.8,
            is_speech: true,
        };
        let silence = VadResult {
            probability: 0.1,
            is_speech: false,
        };

        // Start speech
        assert!(state.update(&speech, 512).is_none());
        assert!(state.is_speech());

        // Continue speech
        assert!(state.update(&speech, 512).is_none());

        // Brief silence (not enough to end)
        assert!(state.update(&silence, 512).is_none());
        assert!(state.is_speech()); // Still in speech

        // More silence to trigger end
        assert!(state.update(&silence, 1600).is_some()); // Returns segment
        assert!(!state.is_speech());
    }

    #[test]
    fn test_vad_state_too_short() {
        let config = VadConfig {
            threshold: 0.5,
            min_silence_ms: 100,
            min_speech_ms: 500, // 500ms minimum
            ..Default::default()
        };
        let mut state = VadState::new(config, 16000);

        let speech = VadResult {
            probability: 0.8,
            is_speech: true,
        };
        let silence = VadResult {
            probability: 0.1,
            is_speech: false,
        };

        // Very short speech (only 512 samples = 32ms)
        state.update(&speech, 512);
        // Long silence to end
        let segment = state.update(&silence, 3200);

        // Should return None because speech was too short
        assert!(segment.is_none());
    }
}
