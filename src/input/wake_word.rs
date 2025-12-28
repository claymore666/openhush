//! Wake word detection using openWakeWord ONNX models.
//!
//! Uses a three-stage pipeline:
//! 1. melspectrogram.onnx: Audio (16kHz) → Mel spectrogram (76x32)
//! 2. embedding_model.onnx: Mel spectrogram → Speech embeddings (96-dim)
//! 3. hey_jarvis.onnx: Accumulated embeddings (1536-dim) → Detection probability
//!
//! Models from: <https://github.com/dscripka/openWakeWord>

use crate::config::{Config, WakeWordConfig};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};

/// Sample rate expected by openWakeWord (16kHz)
#[allow(dead_code)] // Public API for consumers
pub const WAKE_WORD_SAMPLE_RATE: u32 = 16000;

/// Samples per frame for mel spectrogram (80ms at 16kHz = 1280 samples)
pub const SAMPLES_PER_FRAME: usize = 1280;

/// Number of mel spectrogram frames per embedding window
const MEL_FRAMES: usize = 76;

/// Number of mel frequency bins
const MEL_BINS: usize = 32;

/// Embedding dimension from the embedding model
const EMBEDDING_DIM: usize = 96;

/// Number of embeddings accumulated for wake word detection (16 * 96 = 1536)
const EMBEDDING_WINDOW: usize = 16;

/// Model file names
const MELSPEC_MODEL: &str = "melspectrogram.onnx";
const EMBEDDING_MODEL: &str = "embedding_model.onnx";
const WAKE_WORD_MODEL: &str = "hey_jarvis_v0.1.onnx";

/// Model download URLs
const MELSPEC_URL: &str =
    "https://github.com/dscripka/openWakeWord/releases/download/v0.5.1/melspectrogram.onnx";
const EMBEDDING_URL: &str =
    "https://github.com/dscripka/openWakeWord/releases/download/v0.5.1/embedding_model.onnx";
const WAKE_WORD_URL: &str =
    "https://github.com/dscripka/openWakeWord/releases/download/v0.5.1/hey_jarvis_v0.1.onnx";

#[derive(Error, Debug)]
pub enum WakeWordError {
    #[error("Failed to initialize wake word detector: {0}")]
    InitError(String),

    #[error("Failed to load model: {0}")]
    ModelError(String),

    #[error("Failed to process audio: {0}")]
    ProcessError(String),

    #[error("Model not found: {0}. Run: openhush model download wake-word")]
    ModelNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Event emitted when wake word is detected.
#[derive(Debug, Clone)]
pub struct WakeWordEvent {
    /// Name of the detected wake word
    pub name: String,
    /// Detection score (0.0 - 1.0)
    pub score: f32,
    /// Timestamp when detected
    #[allow(dead_code)] // Available for future timeout tracking
    pub timestamp: std::time::Instant,
}

/// Wake word detector using openWakeWord ONNX models.
pub struct WakeWordDetector {
    /// Mel spectrogram model
    melspec_model: Session,
    /// Speech embedding model
    embedding_model: Session,
    /// Wake word classification model
    wakeword_model: Session,
    /// Configuration
    config: WakeWordConfig,
    /// Audio sample buffer
    sample_buffer: Vec<f32>,
    /// Mel spectrogram frame buffer
    mel_buffer: VecDeque<Vec<f32>>,
    /// Embedding buffer (sliding window)
    embedding_buffer: VecDeque<Vec<f32>>,
}

impl WakeWordDetector {
    /// Create a new wake word detector.
    pub fn new(config: &WakeWordConfig) -> Result<Self, WakeWordError> {
        let models_dir = Self::models_dir()?;

        // Check if models exist
        let melspec_path = models_dir.join(MELSPEC_MODEL);
        let embedding_path = models_dir.join(EMBEDDING_MODEL);
        let wakeword_path = models_dir.join(WAKE_WORD_MODEL);

        if !melspec_path.exists() {
            return Err(WakeWordError::ModelNotFound(MELSPEC_MODEL.to_string()));
        }
        if !embedding_path.exists() {
            return Err(WakeWordError::ModelNotFound(EMBEDDING_MODEL.to_string()));
        }
        if !wakeword_path.exists() {
            return Err(WakeWordError::ModelNotFound(WAKE_WORD_MODEL.to_string()));
        }

        info!("Loading wake word models from {:?}", models_dir);

        // Load models with optimizations
        let melspec_model = Session::builder()
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .commit_from_file(&melspec_path)
            .map_err(|e: ort::Error| WakeWordError::ModelError(format!("melspec: {}", e)))?;

        let embedding_model = Session::builder()
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .commit_from_file(&embedding_path)
            .map_err(|e: ort::Error| WakeWordError::ModelError(format!("embedding: {}", e)))?;

        let wakeword_model = Session::builder()
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| WakeWordError::ModelError(e.to_string()))?
            .commit_from_file(&wakeword_path)
            .map_err(|e: ort::Error| WakeWordError::ModelError(format!("wakeword: {}", e)))?;

        info!(
            "Wake word detector initialized (threshold: {:.2})",
            config.threshold
        );

        Ok(Self {
            melspec_model,
            embedding_model,
            wakeword_model,
            config: config.clone(),
            sample_buffer: Vec::with_capacity(SAMPLES_PER_FRAME * 2),
            mel_buffer: VecDeque::with_capacity(MEL_FRAMES + 10),
            embedding_buffer: VecDeque::with_capacity(EMBEDDING_WINDOW + 2),
        })
    }

    /// Get the models directory path
    fn models_dir() -> Result<PathBuf, WakeWordError> {
        Config::data_dir()
            .map(|d| d.join("models").join("wake_word"))
            .map_err(|e| WakeWordError::InitError(e.to_string()))
    }

    /// Check if wake word models are available
    pub fn models_available() -> bool {
        if let Ok(models_dir) = Self::models_dir() {
            models_dir.join(MELSPEC_MODEL).exists()
                && models_dir.join(EMBEDDING_MODEL).exists()
                && models_dir.join(WAKE_WORD_MODEL).exists()
        } else {
            false
        }
    }

    /// Download wake word models
    pub async fn download_models() -> Result<(), WakeWordError> {
        let models_dir = Self::models_dir()?;
        std::fs::create_dir_all(&models_dir)?;

        let downloads = [
            (MELSPEC_URL, MELSPEC_MODEL),
            (EMBEDDING_URL, EMBEDDING_MODEL),
            (WAKE_WORD_URL, WAKE_WORD_MODEL),
        ];

        for (url, name) in downloads {
            let path = models_dir.join(name);
            if !path.exists() {
                info!("Downloading {}...", name);
                Self::download_file(url, &path).await?;
            }
        }

        info!("Wake word models downloaded to {:?}", models_dir);
        Ok(())
    }

    /// Remove wake word models from disk
    pub fn remove_models() -> Result<(), WakeWordError> {
        let models_dir = Self::models_dir()?;

        let models = [MELSPEC_MODEL, EMBEDDING_MODEL, WAKE_WORD_MODEL];
        let mut removed = 0;

        for name in models {
            let path = models_dir.join(name);
            if path.exists() {
                std::fs::remove_file(&path)?;
                info!("Removed {}", name);
                removed += 1;
            }
        }

        // Try to remove the directory if empty
        if models_dir.exists() {
            let _ = std::fs::remove_dir(&models_dir);
        }

        if removed == 0 {
            info!("No wake word models to remove");
        } else {
            info!("Removed {} wake word model(s)", removed);
        }

        Ok(())
    }

    async fn download_file(url: &str, path: &Path) -> Result<(), WakeWordError> {
        use futures_util::StreamExt;
        use std::io::Write;

        let response = reqwest::get(url)
            .await
            .map_err(|e| WakeWordError::ModelError(format!("Download failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(WakeWordError::ModelError(format!(
                "HTTP {}: {}",
                response.status(),
                url
            )));
        }

        let mut file = std::fs::File::create(path)?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| WakeWordError::ModelError(e.to_string()))?;
            file.write_all(&chunk)?;
        }

        Ok(())
    }

    /// Process audio samples and check for wake word.
    ///
    /// Returns Some(WakeWordEvent) if wake word was detected.
    pub fn process(&mut self, samples: &[f32]) -> Option<WakeWordEvent> {
        // Add samples to buffer
        self.sample_buffer.extend_from_slice(samples);

        // Process complete frames
        while self.sample_buffer.len() >= SAMPLES_PER_FRAME {
            // Extract frame
            let frame: Vec<f32> = self.sample_buffer.drain(..SAMPLES_PER_FRAME).collect();

            // Compute mel spectrogram
            if let Ok(mel_frames) = self.compute_melspec(&frame) {
                for mel_frame in mel_frames {
                    self.mel_buffer.push_back(mel_frame);

                    // Keep only what we need for embedding
                    while self.mel_buffer.len() > MEL_FRAMES {
                        self.mel_buffer.pop_front();
                    }
                }
            }

            // Compute embedding when we have enough mel frames
            if self.mel_buffer.len() >= MEL_FRAMES {
                if let Ok(embedding) = self.compute_embedding() {
                    self.embedding_buffer.push_back(embedding);

                    // Keep sliding window
                    while self.embedding_buffer.len() > EMBEDDING_WINDOW {
                        self.embedding_buffer.pop_front();
                    }
                }
            }

            // Check for wake word when we have enough embeddings
            if self.embedding_buffer.len() >= EMBEDDING_WINDOW {
                if let Ok(score) = self.detect_wakeword() {
                    if score >= self.config.threshold {
                        debug!("Wake word detected with score: {:.3}", score);
                        return Some(WakeWordEvent {
                            name: "hey jarvis".to_string(),
                            score,
                            timestamp: std::time::Instant::now(),
                        });
                    }
                }
            }
        }

        None
    }

    /// Compute mel spectrogram from audio frame
    fn compute_melspec(&mut self, samples: &[f32]) -> Result<Vec<Vec<f32>>, WakeWordError> {
        // Create input tensor [1, samples]
        let input = Tensor::from_array(([1, samples.len()], samples.to_vec()))
            .map_err(|e| WakeWordError::ProcessError(format!("create input: {}", e)))?;

        // Run inference
        let outputs = self
            .melspec_model
            .run(ort::inputs!["input" => input])
            .map_err(|e: ort::Error| {
                WakeWordError::ProcessError(format!("melspec inference: {}", e))
            })?;

        // Extract output - shape should be [1, frames, mel_bins]
        let (shape, slice): (&ort::tensor::Shape, &[f32]) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e: ort::Error| WakeWordError::ProcessError(e.to_string()))?;

        if shape.len() < 2 {
            return Err(WakeWordError::ProcessError(
                "Invalid melspec output shape".to_string(),
            ));
        }

        // Convert to vec of frames, applying transformation: spec/10 + 2
        let n_frames = shape[1] as usize;
        let n_bins = if shape.len() > 2 {
            shape[2] as usize
        } else {
            MEL_BINS
        };

        let mut frames = Vec::with_capacity(n_frames);

        for i in 0..n_frames {
            let mut frame = Vec::with_capacity(n_bins);
            for j in 0..n_bins {
                let idx = i * n_bins + j;
                if let Some(val) = slice.get(idx) {
                    frame.push(val / 10.0 + 2.0);
                }
            }
            if frame.len() == n_bins {
                frames.push(frame);
            }
        }

        Ok(frames)
    }

    /// Compute speech embedding from mel spectrogram window
    fn compute_embedding(&mut self) -> Result<Vec<f32>, WakeWordError> {
        // Stack mel frames into input tensor [1, 76, 32, 1]
        let mut input_data = Vec::with_capacity(MEL_FRAMES * MEL_BINS);

        for frame in self.mel_buffer.iter().take(MEL_FRAMES) {
            input_data.extend(frame.iter().take(MEL_BINS));
        }

        // Pad if needed
        while input_data.len() < MEL_FRAMES * MEL_BINS {
            input_data.push(0.0);
        }

        let input = Tensor::from_array(([1, MEL_FRAMES, MEL_BINS, 1], input_data))
            .map_err(|e| WakeWordError::ProcessError(format!("create embedding input: {}", e)))?;

        // Run inference
        let outputs = self
            .embedding_model
            .run(ort::inputs!["input_1" => input])
            .map_err(|e: ort::Error| {
                WakeWordError::ProcessError(format!("embedding inference: {}", e))
            })?;

        // Extract embedding - shape should be [1, 96] or similar
        let (_shape, slice): (&ort::tensor::Shape, &[f32]) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e: ort::Error| WakeWordError::ProcessError(e.to_string()))?;

        Ok(slice.to_vec())
    }

    /// Detect wake word from accumulated embeddings
    fn detect_wakeword(&mut self) -> Result<f32, WakeWordError> {
        // Flatten embeddings into input [1, 1536]
        let mut input_data = Vec::with_capacity(EMBEDDING_WINDOW * EMBEDDING_DIM);

        for embedding in self.embedding_buffer.iter().take(EMBEDDING_WINDOW) {
            input_data.extend(embedding.iter().take(EMBEDDING_DIM));
        }

        // Pad if needed
        while input_data.len() < EMBEDDING_WINDOW * EMBEDDING_DIM {
            input_data.push(0.0);
        }

        let input = Tensor::from_array(([1, EMBEDDING_WINDOW * EMBEDDING_DIM], input_data))
            .map_err(|e| WakeWordError::ProcessError(format!("create wakeword input: {}", e)))?;

        // Run inference
        let outputs = self
            .wakeword_model
            .run(ort::inputs!["input" => input])
            .map_err(|e: ort::Error| {
                WakeWordError::ProcessError(format!("wakeword inference: {}", e))
            })?;

        // Extract probability
        let (_shape, slice): (&ort::tensor::Shape, &[f32]) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e: ort::Error| WakeWordError::ProcessError(e.to_string()))?;

        Ok(slice.first().copied().unwrap_or(0.0))
    }

    /// Reset the detector state (call after detection to avoid repeats).
    pub fn reset(&mut self) {
        self.sample_buffer.clear();
        self.mel_buffer.clear();
        self.embedding_buffer.clear();
    }

    /// Get the configured timeout in seconds.
    #[allow(dead_code)] // Available for future timeout handling
    pub fn timeout_secs(&self) -> f32 {
        self.config.timeout_secs
    }

    /// Check if beep on detect is enabled.
    pub fn beep_enabled(&self) -> bool {
        self.config.beep_on_detect
    }

    /// Check if notification on detect is enabled.
    pub fn notify_enabled(&self) -> bool {
        self.config.notify_on_detect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // WakeWordConfig Tests
    // ===================

    #[test]
    fn test_wake_word_config_defaults() {
        let config = WakeWordConfig::default();
        assert!(!config.enabled);
        assert!(config.model_path.is_none());
        assert!((config.sensitivity - 0.5).abs() < 0.01);
        assert!((config.threshold - 0.5).abs() < 0.01);
        assert!((config.timeout_secs - 10.0).abs() < 0.01);
        assert!(config.beep_on_detect);
        assert!(config.notify_on_detect);
    }

    #[test]
    fn test_wake_word_config_custom() {
        let config = WakeWordConfig {
            enabled: true,
            model_path: Some("/custom/path".to_string()),
            sensitivity: 0.7,
            threshold: 0.8,
            timeout_secs: 15.0,
            beep_on_detect: false,
            notify_on_detect: false,
        };
        assert!(config.enabled);
        assert_eq!(config.model_path, Some("/custom/path".to_string()));
        assert!((config.sensitivity - 0.7).abs() < 0.01);
        assert!((config.threshold - 0.8).abs() < 0.01);
        assert!((config.timeout_secs - 15.0).abs() < 0.01);
        assert!(!config.beep_on_detect);
        assert!(!config.notify_on_detect);
    }

    // ===================
    // WakeWordDetector Tests
    // ===================

    #[test]
    fn test_models_dir() {
        let result = WakeWordDetector::models_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("wake_word"));
    }

    #[test]
    fn test_models_available_without_models() {
        // Unless models are installed, this should return false
        // This test verifies the function doesn't panic
        let _ = WakeWordDetector::models_available();
    }

    #[test]
    fn test_new_with_default_config() {
        let config = WakeWordConfig::default();
        let result = WakeWordDetector::new(&config);
        // Result depends on whether models are installed
        match result {
            Ok(detector) => {
                // Models are installed - verify detector is configured correctly
                assert!((detector.timeout_secs() - 10.0).abs() < 0.01);
                assert!(detector.beep_enabled());
                assert!(detector.notify_enabled());
            }
            Err(WakeWordError::ModelNotFound(name)) => {
                // Models not installed - verify error message
                assert!(
                    name == MELSPEC_MODEL || name == EMBEDDING_MODEL || name == WAKE_WORD_MODEL,
                    "Unexpected missing model: {}",
                    name
                );
            }
            Err(e) => {
                // Other errors (e.g., ONNX runtime issues)
                panic!("Unexpected error: {}", e);
            }
        }
    }

    // ===================
    // WakeWordEvent Tests
    // ===================

    #[test]
    fn test_wake_word_event_creation() {
        let event = WakeWordEvent {
            name: "hey_jarvis".to_string(),
            score: 0.95,
            timestamp: std::time::Instant::now(),
        };
        assert_eq!(event.name, "hey_jarvis");
        assert!((event.score - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_wake_word_event_clone() {
        let event = WakeWordEvent {
            name: "hey_jarvis".to_string(),
            score: 0.85,
            timestamp: std::time::Instant::now(),
        };
        let cloned = event.clone();
        assert_eq!(event.name, cloned.name);
        assert!((event.score - cloned.score).abs() < 0.001);
    }

    // ===================
    // WakeWordError Tests
    // ===================

    #[test]
    fn test_wake_word_error_display() {
        let err = WakeWordError::InitError("test init error".to_string());
        assert_eq!(
            format!("{}", err),
            "Failed to initialize wake word detector: test init error"
        );

        let err = WakeWordError::ModelError("load failed".to_string());
        assert_eq!(format!("{}", err), "Failed to load model: load failed");

        let err = WakeWordError::ProcessError("audio issue".to_string());
        assert_eq!(format!("{}", err), "Failed to process audio: audio issue");

        let err = WakeWordError::ModelNotFound("melspectrogram.onnx".to_string());
        assert!(format!("{}", err).contains("melspectrogram.onnx"));
        assert!(format!("{}", err).contains("openhush model download"));
    }

    // ===================
    // Constants Tests
    // ===================

    #[test]
    fn test_constants() {
        assert_eq!(WAKE_WORD_SAMPLE_RATE, 16000);
        assert_eq!(SAMPLES_PER_FRAME, 1280); // 80ms at 16kHz
        assert_eq!(MEL_FRAMES, 76);
        assert_eq!(MEL_BINS, 32);
        assert_eq!(EMBEDDING_DIM, 96);
        assert_eq!(EMBEDDING_WINDOW, 16);
    }

    #[test]
    fn test_model_urls_are_valid() {
        assert!(MELSPEC_URL.starts_with("https://"));
        assert!(MELSPEC_URL.ends_with(".onnx"));
        assert!(EMBEDDING_URL.starts_with("https://"));
        assert!(EMBEDDING_URL.ends_with(".onnx"));
        assert!(WAKE_WORD_URL.starts_with("https://"));
        assert!(WAKE_WORD_URL.ends_with(".onnx"));
    }

    // ===================
    // Integration Tests (require models)
    // ===================

    #[test]
    #[ignore] // Run with: cargo test test_detector_with_models -- --ignored
    fn test_detector_with_models() {
        let config = WakeWordConfig {
            enabled: true,
            sensitivity: 0.5,
            threshold: 0.5,
            ..Default::default()
        };
        let detector = WakeWordDetector::new(&config).expect("Models should be installed");

        assert!((detector.timeout_secs() - 10.0).abs() < 0.01);
        assert!(detector.beep_enabled());
        assert!(detector.notify_enabled());
    }

    #[test]
    #[ignore] // Run with: cargo test test_process_silence -- --ignored
    fn test_process_silence() {
        let config = WakeWordConfig {
            enabled: true,
            threshold: 0.5,
            ..Default::default()
        };
        let mut detector = WakeWordDetector::new(&config).expect("Models should be installed");

        // Process 1 second of silence (16000 samples)
        let silence: Vec<f32> = vec![0.0; 16000];
        let result = detector.process(&silence);

        // Silence should not trigger wake word
        assert!(result.is_none());
    }

    #[test]
    #[ignore] // Run with: cargo test test_reset_clears_buffers -- --ignored
    fn test_reset_clears_buffers() {
        let config = WakeWordConfig {
            enabled: true,
            ..Default::default()
        };
        let mut detector = WakeWordDetector::new(&config).expect("Models should be installed");

        // Process some audio
        let noise: Vec<f32> = (0..SAMPLES_PER_FRAME * 2)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();
        let _ = detector.process(&noise);

        // Reset should not panic and should clear state
        detector.reset();

        // After reset, processing should work normally
        let silence: Vec<f32> = vec![0.0; SAMPLES_PER_FRAME];
        let result = detector.process(&silence);
        assert!(result.is_none());
    }
}
