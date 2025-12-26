//! Long-running recording sessions with continuous transcription.
//!
//! Provides recording from system audio or microphone with:
//! - Continuous chunked transcription
//! - VAD-based natural break detection
//! - Live output mode
//! - File output with multiple formats

#![allow(dead_code)] // Diarization and mixed recording features used in Phase 3

use crate::config::Config;
#[cfg(feature = "diarization")]
use crate::diarization::{DiarizationConfig, DiarizationEngine, DiarizationError};
use crate::engine::whisper::{WhisperEngine, WhisperError, WhisperModel};
use crate::input::system_audio::{AudioSource, SystemAudioCapture, SystemAudioError};
use crate::input::{AudioBuffer, AudioRecorder, AudioRecorderError};
use crate::vad::silero::SileroVad;
use crate::vad::VadConfig;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::signal;
use tracing::{info, warn};

/// Chunk duration for streaming transcription (seconds)
const CHUNK_DURATION_SECS: f32 = 5.0;

/// Overlap between chunks to prevent word boundary issues (seconds)
const CHUNK_OVERLAP_SECS: f32 = 0.5;

/// Minimum silence duration to consider end of speech (ms)
const MIN_SILENCE_MS: u32 = 500;

/// Recording session errors
#[derive(Error, Debug)]
pub enum RecordingError {
    #[error("Audio capture error: {0}")]
    AudioCapture(String),

    #[error("System audio error: {0}")]
    SystemAudio(#[from] SystemAudioError),

    #[error("Whisper error: {0}")]
    Whisper(#[from] WhisperError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(#[from] anyhow::Error),

    #[error("VAD error: {0}")]
    Vad(String),

    #[cfg(feature = "diarization")]
    #[error("Diarization error: {0}")]
    Diarization(#[from] DiarizationError),

    #[error("Model not found: {0}")]
    ModelNotFound(String),
}

impl From<AudioRecorderError> for RecordingError {
    fn from(e: AudioRecorderError) -> Self {
        RecordingError::AudioCapture(e.to_string())
    }
}

/// Output format for recording transcription
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain text, one line per chunk
    #[default]
    Text,
    /// Timestamped text: [00:01:23] Hello...
    Timestamped,
    /// SubRip subtitle format (.srt)
    Srt,
    /// WebVTT subtitle format (.vtt)
    Vtt,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" | "txt" => Ok(Self::Text),
            "timestamped" | "ts" => Ok(Self::Timestamped),
            "srt" | "subrip" => Ok(Self::Srt),
            "vtt" | "webvtt" => Ok(Self::Vtt),
            _ => Err(format!(
                "Unknown format '{}'. Use: text, timestamped, srt, vtt",
                s
            )),
        }
    }
}

/// Recording session configuration
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    /// Audio source
    pub source: AudioSource,
    /// Output file path (None for stdout only)
    pub output_file: Option<String>,
    /// Enable speaker diarization
    pub enable_diarization: bool,
    /// Live mode: print transcription immediately
    pub live_mode: bool,
    /// Output format
    pub output_format: OutputFormat,
}

/// A transcribed segment with timing
#[derive(Debug, Clone)]
pub struct TranscribedSegment {
    /// Start time from recording start (seconds)
    pub start_secs: f32,
    /// End time from recording start (seconds)
    pub end_secs: f32,
    /// Transcribed text
    pub text: String,
    /// Speaker ID (if diarization enabled)
    pub speaker_id: Option<u32>,
}

impl TranscribedSegment {
    /// Format as timestamped text
    pub fn format_timestamped(&self) -> String {
        let start = format_timestamp(self.start_secs);
        if let Some(speaker) = self.speaker_id {
            format!("[{}] Speaker {}: {}", start, speaker, self.text)
        } else {
            format!("[{}] {}", start, self.text)
        }
    }

    /// Format as SRT subtitle entry
    pub fn format_srt(&self, index: usize) -> String {
        let start = format_srt_timestamp(self.start_secs);
        let end = format_srt_timestamp(self.end_secs);
        let text = if let Some(speaker) = self.speaker_id {
            format!("<v Speaker {}>{}", speaker, self.text)
        } else {
            self.text.clone()
        };
        format!("{}\n{} --> {}\n{}\n", index, start, end, text)
    }

    /// Format as VTT cue
    pub fn format_vtt(&self) -> String {
        let start = format_vtt_timestamp(self.start_secs);
        let end = format_vtt_timestamp(self.end_secs);
        let text = if let Some(speaker) = self.speaker_id {
            format!("<v Speaker {}>{}", speaker, self.text)
        } else {
            self.text.clone()
        };
        format!("{} --> {}\n{}\n", start, end, text)
    }
}

/// Format timestamp as HH:MM:SS
fn format_timestamp(secs: f32) -> String {
    let total_secs = secs as u32;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

/// Format timestamp for SRT (HH:MM:SS,mmm)
fn format_srt_timestamp(secs: f32) -> String {
    let total_ms = (secs * 1000.0) as u32;
    let hours = total_ms / 3600000;
    let mins = (total_ms % 3600000) / 60000;
    let secs = (total_ms % 60000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", hours, mins, secs, ms)
}

/// Format timestamp for VTT (HH:MM:SS.mmm)
fn format_vtt_timestamp(secs: f32) -> String {
    let total_ms = (secs * 1000.0) as u32;
    let hours = total_ms / 3600000;
    let mins = (total_ms % 3600000) / 60000;
    let secs = (total_ms % 60000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, ms)
}

/// Audio source wrapper that handles both mic and system audio
enum AudioSourceCapture {
    Microphone(AudioRecorder),
    Monitor(SystemAudioCapture),
}

impl AudioSourceCapture {
    fn extract_samples(&self) -> Vec<f32> {
        match self {
            AudioSourceCapture::Microphone(_recorder) => {
                // For microphone, we need to use the ring buffer
                // This is a simplified version - full implementation would use mark/extract
                vec![]
            }
            AudioSourceCapture::Monitor(capture) => capture.extract_samples(),
        }
    }
}

/// Long-running recording session
pub struct RecordingSession {
    config: RecordingConfig,
    app_config: Config,
    segments: Vec<TranscribedSegment>,
    running: Arc<AtomicBool>,
}

impl RecordingSession {
    /// Create a new recording session
    pub fn new(config: RecordingConfig) -> Result<Self, RecordingError> {
        let app_config = Config::load().map_err(|e| RecordingError::Config(e.into()))?;

        Ok(Self {
            config,
            app_config,
            segments: Vec::new(),
            running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Run the recording session until Ctrl+C
    pub async fn run(mut self) -> Result<(), RecordingError> {
        // Set up Ctrl+C handler
        let running = Arc::clone(&self.running);
        tokio::spawn(async move {
            let _ = signal::ctrl_c().await;
            info!("Received Ctrl+C, stopping recording...");
            running.store(false, Ordering::SeqCst);
        });

        // Initialize Whisper engine
        let data_dir = Config::data_dir().map_err(|e| RecordingError::Config(e.into()))?;
        let model_name = self.app_config.transcription.effective_model();
        let model: WhisperModel = model_name.parse().map_err(|()| {
            RecordingError::ModelNotFound(format!(
                "Unknown model '{}'. Use: tiny, base, small, medium, large-v3",
                model_name
            ))
        })?;
        let model_path = data_dir.join("models").join(model.filename());

        if !model_path.exists() {
            return Err(RecordingError::ModelNotFound(format!(
                "Model not found: {}. Run: openhush model download {}",
                model_path.display(),
                model_name
            )));
        }

        info!("Loading Whisper model: {}", model.filename());
        let engine = WhisperEngine::new(
            &model_path,
            &self.app_config.transcription.language,
            self.app_config.transcription.translate,
            self.app_config.transcription.device.to_lowercase() != "cpu",
        )?;

        // Initialize audio capture based on source
        let capture = match self.config.source {
            AudioSource::Microphone => {
                info!("Recording from microphone...");
                // For now, use system audio with default source
                // Full implementation would use AudioRecorder
                return Err(RecordingError::AudioCapture(
                    "Microphone recording in record mode not yet implemented. Use --source monitor"
                        .into(),
                ));
            }
            AudioSource::Monitor => {
                info!("Recording system audio...");
                SystemAudioCapture::new(None)?
            }
            AudioSource::Both => {
                info!("Recording both microphone and system audio...");
                return Err(RecordingError::AudioCapture(
                    "Mixed recording not yet implemented. Use --source monitor".into(),
                ));
            }
        };

        // Initialize VAD for natural break detection
        let vad_config = VadConfig::default();
        let _vad = SileroVad::new(&vad_config).map_err(|e| RecordingError::Vad(e.to_string()))?;

        // Initialize diarization engine if enabled
        #[cfg(feature = "diarization")]
        let mut diarization_engine = if self.config.enable_diarization {
            let diar_config = DiarizationConfig {
                max_speakers: self.app_config.diarization.max_speakers,
                similarity_threshold: self.app_config.diarization.similarity_threshold,
            };
            info!("Initializing speaker diarization...");
            Some(DiarizationEngine::new(diar_config).await?)
        } else {
            None
        };

        #[cfg(not(feature = "diarization"))]
        let _diarization_engine: Option<()> = None;

        // Print header for VTT format
        if self.config.output_format == OutputFormat::Vtt && self.config.live_mode {
            println!("WEBVTT\n");
        }

        let start_time = Instant::now();
        let mut last_transcribe_time = Instant::now();
        let mut accumulated_samples: Vec<f32> = Vec::new();
        let mut segment_index = 1;

        let mode_str = if self.config.enable_diarization {
            "with diarization"
        } else {
            ""
        };
        info!("Recording started {}. Press Ctrl+C to stop.", mode_str);
        println!(
            "Recording{}... (Ctrl+C to stop)\n",
            if self.config.enable_diarization {
                " with diarization"
            } else {
                ""
            }
        );

        while self.running.load(Ordering::SeqCst) {
            // Collect samples
            let new_samples = capture.extract_samples();
            accumulated_samples.extend(new_samples);

            // Calculate duration of accumulated audio
            let sample_rate = 16000_u32;
            let duration_secs = accumulated_samples.len() as f32 / sample_rate as f32;

            // Check if we have enough audio to transcribe
            let should_transcribe = duration_secs >= CHUNK_DURATION_SECS
                || (duration_secs >= 1.0
                    && last_transcribe_time.elapsed() > Duration::from_secs(10));

            if should_transcribe && !accumulated_samples.is_empty() {
                let chunk_start_secs = start_time.elapsed().as_secs_f32() - duration_secs;

                // Create audio buffer
                let audio = AudioBuffer {
                    samples: accumulated_samples.clone(),
                    sample_rate,
                };

                // Get speaker ID from diarization if enabled
                #[cfg(feature = "diarization")]
                let speaker_id = if let Some(ref mut diar_engine) = diarization_engine {
                    match diar_engine.diarize(&accumulated_samples, sample_rate) {
                        Ok(diar_segments) => {
                            // Use the first speaker in this chunk
                            diar_segments.first().map(|seg| seg.speaker_id)
                        }
                        Err(e) => {
                            warn!("Diarization error: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                #[cfg(not(feature = "diarization"))]
                let speaker_id: Option<u32> = None;

                // Transcribe
                match engine.transcribe(&audio) {
                    Ok(result) => {
                        if !result.text.trim().is_empty() {
                            let segment = TranscribedSegment {
                                start_secs: chunk_start_secs,
                                end_secs: start_time.elapsed().as_secs_f32(),
                                text: result.text.trim().to_string(),
                                speaker_id,
                            };

                            // Output based on format
                            let output = match self.config.output_format {
                                OutputFormat::Text => format!("{}\n", segment.text),
                                OutputFormat::Timestamped => {
                                    format!("{}\n", segment.format_timestamped())
                                }
                                OutputFormat::Srt => segment.format_srt(segment_index),
                                OutputFormat::Vtt => segment.format_vtt(),
                            };

                            if self.config.live_mode {
                                print!("{}", output);
                                std::io::stdout().flush().ok();
                            }

                            self.segments.push(segment);
                            segment_index += 1;
                        }
                    }
                    Err(e) => {
                        warn!("Transcription error: {}", e);
                    }
                }

                // Keep overlap for continuity
                let overlap_samples = (CHUNK_OVERLAP_SECS * sample_rate as f32) as usize;
                if accumulated_samples.len() > overlap_samples {
                    accumulated_samples =
                        accumulated_samples[accumulated_samples.len() - overlap_samples..].to_vec();
                } else {
                    accumulated_samples.clear();
                }

                last_transcribe_time = Instant::now();
            }

            // Small sleep to prevent busy-waiting
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Final transcription of remaining audio
        if !accumulated_samples.is_empty() {
            let chunk_start_secs =
                start_time.elapsed().as_secs_f32() - (accumulated_samples.len() as f32 / 16000.0);

            // Get final speaker ID from diarization if enabled
            #[cfg(feature = "diarization")]
            let final_speaker_id = if let Some(ref mut diar_engine) = diarization_engine {
                match diar_engine.diarize(&accumulated_samples, 16000) {
                    Ok(diar_segments) => diar_segments.first().map(|s| s.speaker_id),
                    Err(_) => None,
                }
            } else {
                None
            };

            #[cfg(not(feature = "diarization"))]
            let final_speaker_id: Option<u32> = None;

            let audio = AudioBuffer {
                samples: accumulated_samples,
                sample_rate: 16000,
            };

            if let Ok(result) = engine.transcribe(&audio) {
                if !result.text.trim().is_empty() {
                    let segment = TranscribedSegment {
                        start_secs: chunk_start_secs,
                        end_secs: start_time.elapsed().as_secs_f32(),
                        text: result.text.trim().to_string(),
                        speaker_id: final_speaker_id,
                    };

                    let output = match self.config.output_format {
                        OutputFormat::Text => format!("{}\n", segment.text),
                        OutputFormat::Timestamped => format!("{}\n", segment.format_timestamped()),
                        OutputFormat::Srt => segment.format_srt(segment_index),
                        OutputFormat::Vtt => segment.format_vtt(),
                    };

                    if self.config.live_mode {
                        print!("{}", output);
                    }

                    self.segments.push(segment);
                }
            }
        }

        let total_duration = start_time.elapsed();
        println!("\n--- Recording stopped ---");
        println!(
            "Duration: {}",
            format_timestamp(total_duration.as_secs_f32())
        );
        println!("Segments: {}", self.segments.len());
        #[cfg(feature = "diarization")]
        if let Some(ref diar_engine) = diarization_engine {
            println!("Speakers detected: {}", diar_engine.speaker_count());
        }

        // Save to file if output path specified
        if let Some(ref output_path) = self.config.output_file {
            self.save_to_file(output_path)?;
            println!("Saved to: {}", output_path);
        }

        Ok(())
    }

    /// Save transcription to file
    fn save_to_file(&self, path: &str) -> Result<(), RecordingError> {
        let mut file = File::create(path)?;

        // Write header for VTT
        if self.config.output_format == OutputFormat::Vtt {
            writeln!(file, "WEBVTT\n")?;
        }

        for (i, segment) in self.segments.iter().enumerate() {
            let output = match self.config.output_format {
                OutputFormat::Text => format!("{}\n", segment.text),
                OutputFormat::Timestamped => format!("{}\n", segment.format_timestamped()),
                OutputFormat::Srt => segment.format_srt(i + 1),
                OutputFormat::Vtt => segment.format_vtt(),
            };
            write!(file, "{}", output)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0.0), "00:00:00");
        assert_eq!(format_timestamp(61.0), "00:01:01");
        assert_eq!(format_timestamp(3661.0), "01:01:01");
    }

    #[test]
    fn test_format_srt_timestamp() {
        assert_eq!(format_srt_timestamp(0.0), "00:00:00,000");
        assert_eq!(format_srt_timestamp(1.5), "00:00:01,500");
        assert_eq!(format_srt_timestamp(3661.123), "01:01:01,123");
    }

    #[test]
    fn test_format_vtt_timestamp() {
        assert_eq!(format_vtt_timestamp(0.0), "00:00:00.000");
        assert_eq!(format_vtt_timestamp(1.5), "00:00:01.500");
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert_eq!("srt".parse::<OutputFormat>().unwrap(), OutputFormat::Srt);
        assert_eq!("vtt".parse::<OutputFormat>().unwrap(), OutputFormat::Vtt);
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_segment_formatting() {
        let segment = TranscribedSegment {
            start_secs: 0.0,
            end_secs: 5.0,
            text: "Hello world".to_string(),
            speaker_id: None,
        };

        assert!(segment.format_timestamped().contains("[00:00:00]"));
        assert!(segment
            .format_srt(1)
            .contains("00:00:00,000 --> 00:00:05,000"));
        assert!(segment
            .format_vtt()
            .contains("00:00:00.000 --> 00:00:05.000"));
    }
}
