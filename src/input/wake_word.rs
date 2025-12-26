//! Wake word detection using Rustpotter.
//!
//! Enables hands-free activation via customizable wake words like "Hey OpenHush".
//! Uses Rustpotter for efficient keyword spotting on audio streams.

use crate::config::WakeWordConfig;
use rustpotter::{Rustpotter, RustpotterConfig, RustpotterDetection, SampleFormat, AudioFmt};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Sample rate expected by Rustpotter (matches Whisper)
pub const WAKE_WORD_SAMPLE_RATE: u32 = 16000;

/// Chunk size for wake word processing (32ms at 16kHz = 512 samples)
pub const WAKE_WORD_CHUNK_SIZE: usize = 512;

#[derive(Error, Debug)]
pub enum WakeWordError {
    #[error("Failed to initialize wake word detector: {0}")]
    InitError(String),

    #[error("Failed to load wake word model: {0}")]
    ModelError(String),

    #[error("Failed to process audio: {0}")]
    ProcessError(String),

    #[error("No wake word model configured")]
    NoModel,
}

/// Event emitted when wake word is detected.
#[derive(Debug, Clone)]
pub struct WakeWordEvent {
    /// Name of the detected wake word
    pub name: String,
    /// Detection score (0.0 - 1.0)
    pub score: f32,
    /// Timestamp when detected
    pub timestamp: std::time::Instant,
}

/// Wake word detector using Rustpotter.
pub struct WakeWordDetector {
    rustpotter: Rustpotter,
    config: WakeWordConfig,
    /// Number of samples per detection frame
    samples_per_frame: usize,
    /// Buffer for accumulating samples until we have enough for a frame
    sample_buffer: Vec<f32>,
}

impl WakeWordDetector {
    /// Create a new wake word detector.
    pub fn new(config: &WakeWordConfig) -> Result<Self, WakeWordError> {
        info!("Initializing wake word detector");

        // Configure Rustpotter
        let mut rp_config = RustpotterConfig::default();

        // Set thresholds based on config
        rp_config.detector.threshold = config.threshold;
        rp_config.detector.avg_threshold = config.threshold;

        // Create Rustpotter instance
        let mut rustpotter = Rustpotter::new(&rp_config)
            .map_err(|e| WakeWordError::InitError(e.to_string()))?;

        // Load wake word model
        if let Some(ref model_path) = config.model_path {
            let path = Path::new(model_path);
            if path.exists() {
                rustpotter
                    .add_wakeword_from_file("custom", path)
                    .map_err(|e| WakeWordError::ModelError(e.to_string()))?;
                info!("Loaded custom wake word model from: {}", model_path);
            } else {
                return Err(WakeWordError::ModelError(format!(
                    "Model file not found: {}",
                    model_path
                )));
            }
        } else {
            // No model configured - user needs to create one
            warn!("No wake word model configured. Use 'openhush wake-word train' to create one.");
            return Err(WakeWordError::NoModel);
        }

        let samples_per_frame = rustpotter.get_samples_per_frame();
        info!(
            "Wake word detector ready (samples per frame: {}, ~{}ms)",
            samples_per_frame,
            samples_per_frame * 1000 / WAKE_WORD_SAMPLE_RATE as usize
        );

        Ok(Self {
            rustpotter,
            config: config.clone(),
            samples_per_frame,
            sample_buffer: Vec::with_capacity(samples_per_frame),
        })
    }

    /// Process audio samples and check for wake word.
    ///
    /// Returns Some(WakeWordEvent) if wake word was detected.
    pub fn process(&mut self, samples: &[f32]) -> Option<WakeWordEvent> {
        // Add samples to buffer
        self.sample_buffer.extend_from_slice(samples);

        // Process complete frames
        while self.sample_buffer.len() >= self.samples_per_frame {
            // Extract frame
            let frame: Vec<f32> = self.sample_buffer.drain(..self.samples_per_frame).collect();

            // Process frame
            if let Some(detection) = self.rustpotter.process_f32(&frame) {
                debug!(
                    "Wake word detected: {} (score: {:.2})",
                    detection.name, detection.score
                );

                // Check if score meets threshold
                if detection.score >= self.config.threshold {
                    return Some(WakeWordEvent {
                        name: detection.name.clone(),
                        score: detection.score,
                        timestamp: std::time::Instant::now(),
                    });
                }
            }
        }

        None
    }

    /// Reset the detector state (call after detection to avoid repeats).
    pub fn reset(&mut self) {
        self.sample_buffer.clear();
    }

    /// Get the configured timeout in seconds.
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

/// Manager for wake word detection running in background.
pub struct WakeWordManager {
    /// Channel to receive wake word events
    event_rx: mpsc::Receiver<WakeWordEvent>,
    /// Handle to the detection task
    _task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WakeWordManager {
    /// Create a new wake word manager that processes audio from a ring buffer.
    ///
    /// The manager runs detection in a background task and sends events
    /// through a channel when wake words are detected.
    pub fn new(
        config: &WakeWordConfig,
        ring_buffer: Arc<crate::input::ring_buffer::AudioRingBuffer>,
    ) -> Result<(Self, mpsc::Sender<()>), WakeWordError> {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

        let mut detector = WakeWordDetector::new(config)?;
        let check_interval = std::time::Duration::from_millis(32); // 32ms chunks

        // Spawn background detection task
        let task_handle = tokio::spawn(async move {
            let mut last_pos = ring_buffer.current_position();
            let mut interval = tokio::time::interval(check_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Get new samples from ring buffer
                        let current_pos = ring_buffer.current_position();
                        if current_pos != last_pos {
                            let samples = ring_buffer.extract_range(last_pos, current_pos);
                            last_pos = current_pos;

                            // Process samples
                            if let Some(event) = detector.process(&samples) {
                                if event_tx.send(event).await.is_err() {
                                    // Receiver dropped, stop detection
                                    break;
                                }
                                // Reset after detection to avoid repeats
                                detector.reset();
                            }
                        }
                    }
                    _ = stop_rx.recv() => {
                        info!("Wake word detection stopped");
                        break;
                    }
                }
            }
        });

        Ok((
            Self {
                event_rx,
                _task_handle: Some(task_handle),
            },
            stop_tx,
        ))
    }

    /// Try to receive a wake word event (non-blocking).
    pub fn try_recv(&mut self) -> Option<WakeWordEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive a wake word event (blocking).
    pub async fn recv(&mut self) -> Option<WakeWordEvent> {
        self.event_rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wake_word_config_defaults() {
        let config = WakeWordConfig::default();
        assert!(!config.enabled);
        assert!(config.model_path.is_none());
        assert!((config.sensitivity - 0.5).abs() < 0.01);
        assert!((config.threshold - 0.5).abs() < 0.01);
        assert!((config.timeout_secs - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_wake_word_event() {
        let event = WakeWordEvent {
            name: "hey open hush".to_string(),
            score: 0.85,
            timestamp: std::time::Instant::now(),
        };
        assert_eq!(event.name, "hey open hush");
        assert!(event.score > 0.8);
    }
}
