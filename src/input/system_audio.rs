//! System audio capture using PulseAudio/PipeWire monitor sources.
//!
//! Captures desktop audio (meetings, calls, media) via PulseAudio monitor sources.
//! Works with both PulseAudio and PipeWire (via PulseAudio compatibility layer).

#![allow(dead_code)]

use libpulse_binding as pulse;
use pulse::context::Context;
use pulse::mainloop::standard::Mainloop;
use pulse::proplist::Proplist;
use pulse::sample::{Format, Spec};
use pulse::stream::{FlagSet, Stream};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Target sample rate for Whisper (16kHz)
pub const SAMPLE_RATE: u32 = 16000;

/// Audio source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioSource {
    /// Default microphone input
    #[default]
    Microphone,
    /// System audio via PulseAudio monitor source
    Monitor,
    /// Both microphone and system audio mixed
    Both,
}

impl std::str::FromStr for AudioSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mic" | "microphone" => Ok(Self::Microphone),
            "monitor" | "system" | "desktop" => Ok(Self::Monitor),
            "both" | "mix" | "all" => Ok(Self::Both),
            _ => Err(format!(
                "Unknown audio source '{}'. Use: mic, monitor, or both",
                s
            )),
        }
    }
}

/// Errors from system audio capture
#[derive(Error, Debug)]
pub enum SystemAudioError {
    #[error("PulseAudio connection failed: {0}")]
    ConnectionFailed(String),

    #[error("No monitor source found")]
    NoMonitorSource,

    #[error("Stream creation failed: {0}")]
    StreamFailed(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

/// Information about a PulseAudio source (input device or monitor)
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Source name (e.g., "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// True if this is a monitor source (system audio)
    pub is_monitor: bool,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
}

/// System audio capture using PulseAudio
pub struct SystemAudioCapture {
    /// Audio samples buffer
    samples: Arc<Mutex<Vec<f32>>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Capture thread handle
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Source being captured
    source_name: String,
}

impl SystemAudioCapture {
    /// Create a new system audio capture from a monitor source.
    ///
    /// If `source_name` is None, uses the default monitor source.
    pub fn new(source_name: Option<&str>) -> Result<Self, SystemAudioError> {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = Arc::clone(&samples);

        // Find the monitor source to use
        let source = if let Some(name) = source_name {
            name.to_string()
        } else {
            // Find default monitor source
            let sources = list_monitor_sources()?;
            sources
                .first()
                .map(|s| s.name.clone())
                .ok_or(SystemAudioError::NoMonitorSource)?
        };

        let source_name_clone = source.clone();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            if let Err(e) = run_capture_loop(&source_name_clone, samples_clone, shutdown_rx) {
                error!("System audio capture error: {}", e);
            }
        });

        info!("System audio capture started from: {}", source);

        Ok(Self {
            samples,
            shutdown_tx: Some(shutdown_tx),
            thread_handle: Some(thread_handle),
            source_name: source,
        })
    }

    /// Get the source name being captured.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Extract captured samples and clear the buffer.
    ///
    /// Returns samples at 16kHz mono f32 format.
    pub fn extract_samples(&self) -> Vec<f32> {
        let mut samples = self.samples.lock().unwrap();
        std::mem::take(&mut *samples)
    }

    /// Get the current buffer length in samples.
    pub fn buffer_len(&self) -> usize {
        self.samples.lock().unwrap().len()
    }

    /// Get the current buffer duration in seconds.
    pub fn buffer_duration_secs(&self) -> f32 {
        self.buffer_len() as f32 / SAMPLE_RATE as f32
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        info!("System audio capture stopped");
    }
}

/// List available PulseAudio monitor sources (for system audio capture).
pub fn list_monitor_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    // Create mainloop and context
    let mainloop =
        Rc::new(RefCell::new(Mainloop::new().ok_or_else(|| {
            SystemAudioError::ConnectionFailed("Mainloop failed".into())
        })?));

    let mut proplist = Proplist::new().unwrap();
    proplist
        .set_str(pulse::proplist::properties::APPLICATION_NAME, "OpenHush")
        .ok();

    let context = Rc::new(RefCell::new(
        Context::new_with_proplist(&*mainloop.borrow(), "OpenHush", &proplist)
            .ok_or_else(|| SystemAudioError::ConnectionFailed("Context failed".into()))?,
    ));

    // Connect to server
    context
        .borrow_mut()
        .connect(None, pulse::context::FlagSet::NOFLAGS, None)
        .map_err(|e| SystemAudioError::ConnectionFailed(format!("{:?}", e)))?;

    // Wait for connection
    loop {
        mainloop.borrow_mut().iterate(true);
        match context.borrow().get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                return Err(SystemAudioError::ConnectionFailed(
                    "Connection terminated".into(),
                ));
            }
            _ => {}
        }
    }

    // Get source list
    let sources_rc = Rc::new(RefCell::new(Vec::new()));
    let sources_clone = Rc::clone(&sources_rc);
    let done = Rc::new(RefCell::new(false));
    let done_clone = Rc::clone(&done);

    let introspect = context.borrow().introspect();
    let _op = introspect.get_source_info_list(move |result| match result {
        pulse::callbacks::ListResult::Item(info) => {
            let source_info = SourceInfo {
                name: info
                    .name
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                description: info
                    .description
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                is_monitor: info.monitor_of_sink.is_some(),
                sample_rate: info.sample_spec.rate,
                channels: info.sample_spec.channels,
            };
            sources_clone.borrow_mut().push(source_info);
        }
        pulse::callbacks::ListResult::End | pulse::callbacks::ListResult::Error => {
            *done_clone.borrow_mut() = true;
        }
    });

    // Wait for operation to complete
    while !*done.borrow() {
        mainloop.borrow_mut().iterate(true);
    }

    let mut sources = sources_rc.borrow().clone();

    // Filter to only monitor sources
    sources.retain(|s| s.is_monitor);

    debug!("Found {} monitor sources", sources.len());
    for source in &sources {
        debug!("  - {} ({})", source.name, source.description);
    }

    Ok(sources)
}

/// List all PulseAudio sources (including microphones).
pub fn list_all_sources() -> Result<Vec<SourceInfo>, SystemAudioError> {
    let mainloop =
        Rc::new(RefCell::new(Mainloop::new().ok_or_else(|| {
            SystemAudioError::ConnectionFailed("Mainloop failed".into())
        })?));

    let mut proplist = Proplist::new().unwrap();
    proplist
        .set_str(pulse::proplist::properties::APPLICATION_NAME, "OpenHush")
        .ok();

    let context = Rc::new(RefCell::new(
        Context::new_with_proplist(&*mainloop.borrow(), "OpenHush", &proplist)
            .ok_or_else(|| SystemAudioError::ConnectionFailed("Context failed".into()))?,
    ));

    context
        .borrow_mut()
        .connect(None, pulse::context::FlagSet::NOFLAGS, None)
        .map_err(|e| SystemAudioError::ConnectionFailed(format!("{:?}", e)))?;

    loop {
        mainloop.borrow_mut().iterate(true);
        match context.borrow().get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                return Err(SystemAudioError::ConnectionFailed(
                    "Connection terminated".into(),
                ));
            }
            _ => {}
        }
    }

    let sources_rc = Rc::new(RefCell::new(Vec::new()));
    let sources_clone = Rc::clone(&sources_rc);
    let done = Rc::new(RefCell::new(false));
    let done_clone = Rc::clone(&done);

    let introspect = context.borrow().introspect();
    let _op = introspect.get_source_info_list(move |result| match result {
        pulse::callbacks::ListResult::Item(info) => {
            let source_info = SourceInfo {
                name: info
                    .name
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                description: info
                    .description
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                is_monitor: info.monitor_of_sink.is_some(),
                sample_rate: info.sample_spec.rate,
                channels: info.sample_spec.channels,
            };
            sources_clone.borrow_mut().push(source_info);
        }
        pulse::callbacks::ListResult::End | pulse::callbacks::ListResult::Error => {
            *done_clone.borrow_mut() = true;
        }
    });

    while !*done.borrow() {
        mainloop.borrow_mut().iterate(true);
    }

    let sources = sources_rc.borrow().clone();

    Ok(sources)
}

/// Run the audio capture loop (called in a separate thread).
fn run_capture_loop(
    source_name: &str,
    samples: Arc<Mutex<Vec<f32>>>,
    shutdown_rx: mpsc::Receiver<()>,
) -> Result<(), SystemAudioError> {
    let mainloop =
        Rc::new(RefCell::new(Mainloop::new().ok_or_else(|| {
            SystemAudioError::ConnectionFailed("Mainloop failed".into())
        })?));

    let mut proplist = Proplist::new().unwrap();
    proplist
        .set_str(pulse::proplist::properties::APPLICATION_NAME, "OpenHush")
        .ok();
    proplist
        .set_str(
            pulse::proplist::properties::APPLICATION_ID,
            "org.openhush.recorder",
        )
        .ok();

    let context = Rc::new(RefCell::new(
        Context::new_with_proplist(&*mainloop.borrow(), "OpenHush", &proplist)
            .ok_or_else(|| SystemAudioError::ConnectionFailed("Context failed".into()))?,
    ));

    context
        .borrow_mut()
        .connect(None, pulse::context::FlagSet::NOFLAGS, None)
        .map_err(|e| SystemAudioError::ConnectionFailed(format!("{:?}", e)))?;

    // Wait for connection
    loop {
        mainloop.borrow_mut().iterate(true);
        match context.borrow().get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                return Err(SystemAudioError::ConnectionFailed(
                    "Connection terminated".into(),
                ));
            }
            _ => {}
        }
    }

    // Create recording stream
    let spec = Spec {
        format: Format::F32le,
        channels: 1,
        rate: SAMPLE_RATE,
    };

    if !spec.is_valid() {
        return Err(SystemAudioError::StreamFailed("Invalid sample spec".into()));
    }

    let stream = Rc::new(RefCell::new(
        Stream::new(&mut context.borrow_mut(), "OpenHush Recorder", &spec, None)
            .ok_or_else(|| SystemAudioError::StreamFailed("Stream creation failed".into()))?,
    ));

    // Set up read callback
    let samples_clone = Arc::clone(&samples);
    let stream_clone = Rc::clone(&stream);
    stream
        .borrow_mut()
        .set_read_callback(Some(Box::new(move |len| {
            if len == 0 {
                return;
            }

            let mut stream = stream_clone.borrow_mut();
            match stream.peek() {
                Ok(pulse::stream::PeekResult::Data(data)) => {
                    // Convert bytes to f32 samples
                    let float_samples: Vec<f32> = data
                        .chunks_exact(4)
                        .map(|chunk| {
                            let bytes: [u8; 4] = chunk.try_into().unwrap();
                            f32::from_le_bytes(bytes)
                        })
                        .collect();

                    // Append to buffer
                    if let Ok(mut buffer) = samples_clone.lock() {
                        buffer.extend(float_samples);
                    }

                    let _ = stream.discard();
                }
                Ok(pulse::stream::PeekResult::Hole(_)) => {
                    let _ = stream.discard();
                }
                Ok(pulse::stream::PeekResult::Empty) => {}
                Err(e) => {
                    warn!("Stream peek error: {:?}", e);
                }
            }
        })));

    // Connect to the source
    stream
        .borrow_mut()
        .connect_record(
            Some(source_name),
            None,
            FlagSet::ADJUST_LATENCY | FlagSet::AUTO_TIMING_UPDATE,
        )
        .map_err(|e| SystemAudioError::StreamFailed(format!("Connect failed: {:?}", e)))?;

    // Wait for stream to be ready
    loop {
        mainloop.borrow_mut().iterate(true);
        match stream.borrow().get_state() {
            pulse::stream::State::Ready => break,
            pulse::stream::State::Failed | pulse::stream::State::Terminated => {
                return Err(SystemAudioError::StreamFailed("Stream terminated".into()));
            }
            _ => {}
        }
    }

    info!("Recording from: {}", source_name);

    // Main loop - process audio until shutdown signal
    loop {
        // Check for shutdown
        if shutdown_rx.try_recv().is_ok() {
            debug!("Shutdown signal received");
            break;
        }

        // Process PulseAudio events with timeout
        mainloop.borrow_mut().iterate(false);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Cleanup
    stream.borrow_mut().disconnect().ok();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // AudioSource Tests
    // ===================

    #[test]
    fn test_audio_source_from_str() {
        assert_eq!(
            "mic".parse::<AudioSource>().unwrap(),
            AudioSource::Microphone
        );
        assert_eq!(
            "monitor".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );
        assert_eq!("both".parse::<AudioSource>().unwrap(), AudioSource::Both);
        assert!("invalid".parse::<AudioSource>().is_err());
    }

    #[test]
    fn test_audio_source_from_str_aliases() {
        // Test all aliases for mic
        assert_eq!(
            "microphone".parse::<AudioSource>().unwrap(),
            AudioSource::Microphone
        );

        // Test all aliases for monitor
        assert_eq!(
            "system".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );
        assert_eq!(
            "desktop".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );

        // Test all aliases for both
        assert_eq!("mix".parse::<AudioSource>().unwrap(), AudioSource::Both);
        assert_eq!("all".parse::<AudioSource>().unwrap(), AudioSource::Both);
    }

    #[test]
    fn test_audio_source_from_str_case_insensitive() {
        assert_eq!(
            "MIC".parse::<AudioSource>().unwrap(),
            AudioSource::Microphone
        );
        assert_eq!(
            "MONITOR".parse::<AudioSource>().unwrap(),
            AudioSource::Monitor
        );
        assert_eq!("Both".parse::<AudioSource>().unwrap(), AudioSource::Both);
    }

    #[test]
    fn test_audio_source_from_str_error_message() {
        let result = "invalid".parse::<AudioSource>();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("invalid"));
        assert!(err.contains("mic"));
        assert!(err.contains("monitor"));
        assert!(err.contains("both"));
    }

    #[test]
    fn test_audio_source_default() {
        assert_eq!(AudioSource::default(), AudioSource::Microphone);
    }

    #[test]
    fn test_audio_source_eq() {
        assert_eq!(AudioSource::Microphone, AudioSource::Microphone);
        assert_eq!(AudioSource::Monitor, AudioSource::Monitor);
        assert_eq!(AudioSource::Both, AudioSource::Both);
        assert_ne!(AudioSource::Microphone, AudioSource::Monitor);
    }

    #[test]
    fn test_audio_source_clone() {
        let source = AudioSource::Monitor;
        let cloned = source;
        assert_eq!(source, cloned);
    }

    // ===================
    // SystemAudioError Tests
    // ===================

    #[test]
    fn test_system_audio_error_display() {
        let err = SystemAudioError::ConnectionFailed("timeout".to_string());
        assert_eq!(format!("{}", err), "PulseAudio connection failed: timeout");

        let err = SystemAudioError::NoMonitorSource;
        assert_eq!(format!("{}", err), "No monitor source found");

        let err = SystemAudioError::StreamFailed("buffer error".to_string());
        assert_eq!(format!("{}", err), "Stream creation failed: buffer error");

        let err = SystemAudioError::OperationFailed("read error".to_string());
        assert_eq!(format!("{}", err), "Operation failed: read error");
    }

    // ===================
    // SourceInfo Tests
    // ===================

    #[test]
    fn test_source_info_creation() {
        let info = SourceInfo {
            name: "test_source".to_string(),
            description: "Test Source".to_string(),
            is_monitor: true,
            sample_rate: 48000,
            channels: 2,
        };
        assert_eq!(info.name, "test_source");
        assert_eq!(info.description, "Test Source");
        assert!(info.is_monitor);
        assert_eq!(info.sample_rate, 48000);
        assert_eq!(info.channels, 2);
    }

    #[test]
    fn test_source_info_clone() {
        let info = SourceInfo {
            name: "monitor".to_string(),
            description: "Monitor of Built-in Audio".to_string(),
            is_monitor: true,
            sample_rate: 44100,
            channels: 2,
        };
        let cloned = info.clone();
        assert_eq!(info.name, cloned.name);
        assert_eq!(info.description, cloned.description);
        assert_eq!(info.is_monitor, cloned.is_monitor);
        assert_eq!(info.sample_rate, cloned.sample_rate);
        assert_eq!(info.channels, cloned.channels);
    }

    // ===================
    // Constants Tests
    // ===================

    #[test]
    fn test_sample_rate_constant() {
        assert_eq!(SAMPLE_RATE, 16000);
    }

    // ===================
    // Integration Tests (require PulseAudio/PipeWire)
    // ===================

    #[test]
    #[ignore] // Run with: cargo test test_list_monitor_sources -- --ignored
    fn test_list_monitor_sources() {
        let result = list_monitor_sources();
        // Should succeed if PulseAudio is available
        if let Ok(sources) = result {
            // All returned sources should be monitors
            for source in &sources {
                assert!(source.is_monitor, "Source {} is not a monitor", source.name);
            }
        }
    }

    #[test]
    #[ignore] // Run with: cargo test test_list_all_sources -- --ignored
    fn test_list_all_sources() {
        let result = list_all_sources();
        // Should succeed if PulseAudio is available
        if let Ok(sources) = result {
            assert!(!sources.is_empty(), "Should have at least one source");
            // Check that we have valid sample rates
            for source in &sources {
                assert!(source.sample_rate > 0, "Invalid sample rate");
                assert!(source.channels > 0, "Invalid channel count");
            }
        }
    }

    #[test]
    #[ignore] // Run with: cargo test test_system_audio_capture_new -- --ignored
    fn test_system_audio_capture_new() {
        let result = SystemAudioCapture::new(None);
        match result {
            Ok(capture) => {
                // Should have a valid source name
                assert!(!capture.source_name().is_empty());
                // Initial buffer should be empty
                assert_eq!(capture.buffer_len(), 0);
            }
            Err(SystemAudioError::NoMonitorSource) => {
                // OK - no monitor sources available
            }
            Err(e) => {
                // PulseAudio might not be available
                eprintln!("System audio capture failed (expected in CI): {}", e);
            }
        }
    }

    #[test]
    #[ignore] // Run with: cargo test test_system_audio_capture_extract -- --ignored
    fn test_system_audio_capture_extract() {
        if let Ok(capture) = SystemAudioCapture::new(None) {
            // Wait briefly for some audio
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Extract samples (may be empty if no audio playing)
            let samples = capture.extract_samples();
            // All samples should be valid floats
            for &sample in &samples {
                assert!(sample.is_finite(), "Sample should be finite");
                assert!(
                    sample >= -1.0 && sample <= 1.0,
                    "Sample should be normalized"
                );
            }

            // Buffer should be cleared after extract
            assert_eq!(capture.buffer_len(), 0);
        }
    }

    #[test]
    #[ignore] // Run with: cargo test test_buffer_duration -- --ignored
    fn test_buffer_duration() {
        if let Ok(capture) = SystemAudioCapture::new(None) {
            // Initial duration should be 0
            let duration = capture.buffer_duration_secs();
            assert!(duration >= 0.0);

            // Wait for some audio
            std::thread::sleep(std::time::Duration::from_millis(200));

            // Duration should increase with captured audio
            let duration = capture.buffer_duration_secs();
            // May still be 0 if no system audio is playing
            assert!(duration >= 0.0);
        }
    }
}
