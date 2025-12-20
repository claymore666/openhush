//! Silero VAD implementation using silero-vad-rust crate.
//!
//! Silero VAD is a lightweight LSTM-based voice activity detector.
//! This implementation uses the silero-vad-rust crate which bundles
//! the ONNX model and provides a simple API.
//!
//! Model specifications:
//! - Input: 512 samples at 16kHz (32ms chunks)
//! - Output: Speech probability [0.0, 1.0]

use super::{VadConfig, VadEngine, VadError, VadResult};
use silero_vad_rust::silero_vad::model::{load_silero_vad, OnnxModel};

/// Silero VAD chunk size in samples (512 samples = 32ms at 16kHz)
pub const SILERO_CHUNK_SIZE: usize = 512;

/// Silero VAD sample rate
pub const SILERO_SAMPLE_RATE: u32 = 16000;

/// Silero VAD engine using silero-vad-rust crate.
pub struct SileroVad {
    model: OnnxModel,
    threshold: f32,
}

impl SileroVad {
    /// Create a new Silero VAD instance.
    pub fn new(config: &VadConfig) -> Result<Self, VadError> {
        let model = load_silero_vad()
            .map_err(|e| VadError::ModelLoad(format!("{:?}", e)))?;

        Ok(Self {
            model,
            threshold: config.threshold,
        })
    }
}

impl VadEngine for SileroVad {
    fn process(&mut self, samples: &[f32]) -> Result<VadResult, VadError> {
        // Process chunks and average the probabilities
        if samples.is_empty() {
            return Ok(VadResult {
                probability: 0.0,
                is_speech: false,
            });
        }

        let mut total_prob = 0.0;
        let mut count = 0;

        for chunk in samples.chunks(SILERO_CHUNK_SIZE) {
            if chunk.len() == SILERO_CHUNK_SIZE {
                // Process full chunk
                match self.model.forward_chunk(chunk, SILERO_SAMPLE_RATE) {
                    Ok(prob_array) => {
                        // Extract the first (and only) probability value from the array
                        if let Some(&prob) = prob_array.iter().next() {
                            total_prob += prob;
                            count += 1;
                        }
                    }
                    Err(e) => {
                        return Err(VadError::Inference(format!("{:?}", e)));
                    }
                }
            } else if !chunk.is_empty() {
                // Pad partial chunk
                let mut padded = vec![0.0f32; SILERO_CHUNK_SIZE];
                padded[..chunk.len()].copy_from_slice(chunk);
                match self.model.forward_chunk(&padded, SILERO_SAMPLE_RATE) {
                    Ok(prob_array) => {
                        if let Some(&prob) = prob_array.iter().next() {
                            total_prob += prob;
                            count += 1;
                        }
                    }
                    Err(e) => {
                        return Err(VadError::Inference(format!("{:?}", e)));
                    }
                }
            }
        }

        let probability = if count > 0 {
            total_prob / count as f32
        } else {
            0.0
        };

        Ok(VadResult {
            probability,
            is_speech: probability >= self.threshold,
        })
    }

    fn reset(&mut self) {
        self.model.reset_states();
    }

    fn chunk_size(&self) -> usize {
        SILERO_CHUNK_SIZE
    }

    fn sample_rate(&self) -> u32 {
        SILERO_SAMPLE_RATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> VadConfig {
        VadConfig {
            enabled: true,
            threshold: 0.5,
            ..Default::default()
        }
    }

    #[test]
    fn test_silero_vad_creation() {
        let config = test_config();
        let vad = SileroVad::new(&config);
        assert!(vad.is_ok(), "Failed to create Silero VAD: {:?}", vad.err());
    }

    #[test]
    fn test_silero_vad_silence() {
        let config = test_config();
        let mut vad = SileroVad::new(&config).unwrap();

        // Process silence
        let silence = vec![0.0f32; SILERO_CHUNK_SIZE];
        let result = vad.process(&silence).unwrap();

        // Silence should have low probability
        assert!(
            result.probability < 0.5,
            "Silence probability too high: {}",
            result.probability
        );
        assert!(!result.is_speech);
    }

    #[test]
    fn test_silero_vad_reset() {
        let config = test_config();
        let mut vad = SileroVad::new(&config).unwrap();

        // Process some audio
        let samples = vec![0.1f32; SILERO_CHUNK_SIZE];
        let _ = vad.process(&samples);

        // Reset should not panic
        vad.reset();
    }

    #[test]
    fn test_silero_vad_chunk_size() {
        let config = test_config();
        let vad = SileroVad::new(&config).unwrap();
        assert_eq!(vad.chunk_size(), SILERO_CHUNK_SIZE);
        assert_eq!(vad.sample_rate(), SILERO_SAMPLE_RATE);
    }

    #[test]
    fn test_silero_vad_partial_chunk() {
        let config = test_config();
        let mut vad = SileroVad::new(&config).unwrap();

        // Process partial chunk (should be padded)
        let samples = vec![0.0f32; 256];
        let result = vad.process(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_silero_vad_multiple_chunks() {
        let config = test_config();
        let mut vad = SileroVad::new(&config).unwrap();

        // Process multiple chunks worth of samples
        let samples = vec![0.0f32; SILERO_CHUNK_SIZE * 3];
        let result = vad.process(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_silero_vad_empty() {
        let config = test_config();
        let mut vad = SileroVad::new(&config).unwrap();

        // Process empty input
        let samples: Vec<f32> = vec![];
        let result = vad.process(&samples).unwrap();
        assert_eq!(result.probability, 0.0);
        assert!(!result.is_speech);
    }
}
