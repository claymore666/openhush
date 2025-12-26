//! Speaker diarization using pyannote-rs.
//!
//! Identifies who is speaking when in audio recordings using:
//! - Segmentation model: detects when speech occurs
//! - Speaker embedding model: identifies who is speaking
//!
//! Models are downloaded on first use to the data directory.

#![allow(dead_code)] // Integration with recording module in Phase 5

use crate::config::Config;
use pyannote_rs::{EmbeddingExtractor, EmbeddingManager};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Sample rate expected by pyannote models (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// Default similarity threshold for speaker matching
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.5;

/// Model file names
pub const SEGMENTATION_MODEL: &str = "segmentation-3.0.onnx";
pub const EMBEDDING_MODEL: &str = "wespeaker_en_voxceleb_CAM++.onnx";

/// Model download URLs
const SEGMENTATION_URL: &str =
    "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx";
const EMBEDDING_URL: &str = "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/wespeaker_en_voxceleb_CAM++.onnx";

/// Diarization errors
#[derive(Error, Debug)]
pub enum DiarizationError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model download failed: {0}")]
    DownloadFailed(String),

    #[error("Segmentation failed: {0}")]
    SegmentationFailed(String),

    #[error("Embedding extraction failed: {0}")]
    EmbeddingFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),
}

/// A speaker segment with timing and speaker ID
#[derive(Debug, Clone)]
pub struct SpeakerSegment {
    /// Start time in seconds from recording start
    pub start_secs: f32,
    /// End time in seconds
    pub end_secs: f32,
    /// Speaker ID (1-based)
    pub speaker_id: u32,
    /// Audio samples for this segment
    pub samples: Vec<f32>,
}

/// Diarization configuration
#[derive(Debug, Clone)]
pub struct DiarizationConfig {
    /// Maximum number of speakers to detect
    pub max_speakers: usize,
    /// Similarity threshold for speaker matching (0.0 - 1.0)
    pub similarity_threshold: f32,
}

impl Default for DiarizationConfig {
    fn default() -> Self {
        Self {
            max_speakers: 6,
            similarity_threshold: DEFAULT_SIMILARITY_THRESHOLD,
        }
    }
}

/// Speaker diarization engine
pub struct DiarizationEngine {
    /// Speaker embedding extractor
    extractor: EmbeddingExtractor,
    /// Speaker embedding manager for clustering
    manager: EmbeddingManager,
    /// Path to segmentation model
    segmentation_model_path: PathBuf,
    /// Configuration
    config: DiarizationConfig,
}

impl DiarizationEngine {
    /// Create a new diarization engine.
    ///
    /// Downloads models if not present.
    pub async fn new(config: DiarizationConfig) -> Result<Self, DiarizationError> {
        let models_dir = Self::models_dir()?;
        std::fs::create_dir_all(&models_dir)?;

        // Ensure models are downloaded
        Self::ensure_models(&models_dir).await?;

        let segmentation_path = models_dir.join(SEGMENTATION_MODEL);
        let embedding_path = models_dir.join(EMBEDDING_MODEL);

        let extractor = EmbeddingExtractor::new(embedding_path.to_str().unwrap())
            .map_err(|e| DiarizationError::EmbeddingFailed(format!("{:?}", e)))?;

        let manager = EmbeddingManager::new(config.max_speakers);

        info!(
            "Diarization engine initialized with max {} speakers",
            config.max_speakers
        );

        Ok(Self {
            extractor,
            manager,
            segmentation_model_path: segmentation_path,
            config,
        })
    }

    /// Get the models directory path
    fn models_dir() -> Result<PathBuf, DiarizationError> {
        Config::data_dir()
            .map(|d| d.join("models").join("diarization"))
            .map_err(|e| DiarizationError::Config(e.to_string()))
    }

    /// Check if diarization models are available
    pub fn models_available() -> bool {
        if let Ok(models_dir) = Self::models_dir() {
            models_dir.join(SEGMENTATION_MODEL).exists()
                && models_dir.join(EMBEDDING_MODEL).exists()
        } else {
            false
        }
    }

    /// Ensure models are downloaded
    async fn ensure_models(models_dir: &Path) -> Result<(), DiarizationError> {
        let segmentation_path = models_dir.join(SEGMENTATION_MODEL);
        let embedding_path = models_dir.join(EMBEDDING_MODEL);

        if !segmentation_path.exists() {
            info!("Downloading segmentation model...");
            Self::download_model(SEGMENTATION_URL, &segmentation_path).await?;
        }

        if !embedding_path.exists() {
            info!("Downloading speaker embedding model...");
            Self::download_model(EMBEDDING_URL, &embedding_path).await?;
        }

        Ok(())
    }

    /// Download a model file
    async fn download_model(url: &str, path: &Path) -> Result<(), DiarizationError> {
        use futures_util::StreamExt;
        use std::io::Write;

        let response = reqwest::get(url)
            .await
            .map_err(|e| DiarizationError::DownloadFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DiarizationError::DownloadFailed(format!(
                "HTTP {}: {}",
                response.status(),
                url
            )));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = std::fs::File::create(path)?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| DiarizationError::DownloadFailed(e.to_string()))?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            if total_size > 0 {
                let percent = (downloaded as f64 / total_size as f64 * 100.0) as u32;
                debug!("Download progress: {}%", percent);
            }
        }

        info!("Downloaded: {}", path.display());
        Ok(())
    }

    /// Process audio and return speaker segments.
    ///
    /// Takes audio samples at 16kHz mono and returns segments with speaker IDs.
    /// Note: pyannote-rs expects i16 samples, so f32 samples are converted.
    pub fn diarize(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        if sample_rate != SAMPLE_RATE {
            warn!(
                "Sample rate mismatch: got {} Hz, expected {} Hz",
                sample_rate, SAMPLE_RATE
            );
        }

        // Convert f32 samples to i16 (pyannote-rs expects i16)
        let samples_i16: Vec<i16> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();

        let segments = pyannote_rs::get_segments(
            &samples_i16,
            sample_rate,
            self.segmentation_model_path.to_str().unwrap(),
        )
        .map_err(|e| DiarizationError::SegmentationFailed(format!("{:?}", e)))?;

        let mut speaker_segments = Vec::new();

        for segment_result in segments {
            match segment_result {
                Ok(segment) => {
                    // Compute speaker embedding
                    match self.extractor.compute(&segment.samples) {
                        Ok(embedding) => {
                            let embedding_vec: Vec<f32> = embedding.collect();

                            // Find or create speaker
                            let speaker_id = if self.manager.get_all_speakers().len()
                                >= self.config.max_speakers
                            {
                                // At max speakers, find best match
                                self.manager
                                    .get_best_speaker_match(embedding_vec)
                                    .unwrap_or(0) as u32
                                    + 1
                            } else {
                                // Try to match existing speaker or create new
                                (self
                                    .manager
                                    .search_speaker(embedding_vec, self.config.similarity_threshold)
                                    .unwrap_or(0)
                                    + 1) as u32
                            };

                            // Convert i16 samples back to f32
                            let samples_f32: Vec<f32> = segment
                                .samples
                                .iter()
                                .map(|&s| s as f32 / i16::MAX as f32)
                                .collect();

                            speaker_segments.push(SpeakerSegment {
                                start_secs: segment.start as f32,
                                end_secs: segment.end as f32,
                                speaker_id,
                                samples: samples_f32,
                            });

                            debug!(
                                "Segment {:.2}s - {:.2}s: Speaker {}",
                                segment.start, segment.end, speaker_id
                            );
                        }
                        Err(e) => {
                            warn!("Failed to extract embedding: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to process segment: {:?}", e);
                }
            }
        }

        info!(
            "Diarization complete: {} segments, {} unique speakers",
            speaker_segments.len(),
            self.manager.get_all_speakers().len()
        );

        Ok(speaker_segments)
    }

    /// Reset speaker tracking (for new recording sessions)
    pub fn reset(&mut self) {
        self.manager = EmbeddingManager::new(self.config.max_speakers);
        info!("Diarization engine reset");
    }

    /// Get the number of detected speakers
    pub fn speaker_count(&self) -> usize {
        self.manager.get_all_speakers().len()
    }
}

/// Download diarization models (CLI command support)
pub async fn download_models() -> Result<(), DiarizationError> {
    let models_dir = DiarizationEngine::models_dir()?;
    std::fs::create_dir_all(&models_dir)?;
    DiarizationEngine::ensure_models(&models_dir).await?;
    println!("Diarization models downloaded to: {}", models_dir.display());
    Ok(())
}

/// Check if diarization models are installed
pub fn check_models() -> bool {
    DiarizationEngine::models_available()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diarization_config_default() {
        let config = DiarizationConfig::default();
        assert_eq!(config.max_speakers, 6);
        assert!((config.similarity_threshold - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_models_dir() {
        let result = DiarizationEngine::models_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("diarization"));
    }
}
