# Key Interfaces

## Platform Traits (src/platform/mod.rs)

```rust
/// Hotkey events
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// Global hotkey handling
pub trait HotkeyHandler: Send + Sync {
    fn start(&mut self, key: &str) -> Result<(), PlatformError>;
    fn stop(&mut self) -> Result<(), PlatformError>;
    fn poll(&mut self) -> Option<HotkeyEvent>;
}

/// Text output (clipboard + paste)
pub trait TextOutput: Send + Sync {
    fn copy_to_clipboard(&self, text: &str) -> Result<(), PlatformError>;
    fn paste_text(&self, text: &str) -> Result<(), PlatformError>;
}

/// Desktop notifications
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), PlatformError>;
}

/// Audio feedback (beeps)
pub trait AudioFeedback: Send + Sync {
    fn play_start_sound(&self) -> Result<(), PlatformError>;
    fn play_stop_sound(&self) -> Result<(), PlatformError>;
}

/// Combined platform interface
pub trait Platform: HotkeyHandler + TextOutput + Notifier + AudioFeedback {
    fn display_server(&self) -> &str;
    fn is_tty(&self) -> bool;
}
```

## Queue Interface (src/queue/)

```rust
/// Recording queue
pub trait RecordingQueue: Send + Sync {
    /// Add recording, returns assigned sequence ID
    fn enqueue(&self, audio: Vec<f32>) -> Result<u64, QueueError>;

    /// Get next recording for processing
    fn dequeue(&self) -> Option<Recording>;

    /// Current queue depth
    fn pending_count(&self) -> usize;
}

/// Result aggregator (maintains order)
pub trait ResultAggregator: Send + Sync {
    /// Submit a transcription result
    fn submit(&self, result: TranscriptionResult);

    /// Get next result in sequence order (blocks if gap)
    fn next_ordered(&self) -> Option<TranscriptionResult>;

    /// Get all ready results (non-blocking)
    fn drain_ready(&self) -> Vec<TranscriptionResult>;
}
```

## GPU Interface (src/gpu/)

```rust
/// GPU pool for multi-GPU transcription
pub trait GpuPool: Send + Sync {
    /// Initialize pool, load models on all GPUs
    fn init(&mut self, model_path: &Path) -> Result<(), GpuError>;

    /// Get available GPU count
    fn gpu_count(&self) -> usize;

    /// Submit job, returns immediately
    fn submit(&self, recording: Recording) -> Result<(), GpuError>;

    /// Receive completed results
    fn receive(&self) -> Option<TranscriptionResult>;
}

/// Individual GPU worker
pub trait GpuWorker: Send {
    fn device_id(&self) -> u32;
    fn is_busy(&self) -> bool;
    fn transcribe(&mut self, audio: &[f32]) -> Result<String, GpuError>;
}
```

## Engine Interface (src/engine/)

```rust
/// Whisper transcription
pub trait Transcriber: Send + Sync {
    fn transcribe(&self, audio: &[f32], language: &str) -> Result<String, EngineError>;
    fn model_name(&self) -> &str;
}

/// Voice activity detection
pub trait VoiceDetector: Send + Sync {
    /// Returns (start_sample, end_sample) of voice activity
    fn detect(&self, audio: &[f32]) -> Option<(usize, usize)>;

    /// Trim silence from audio
    fn trim_silence(&self, audio: &[f32]) -> Vec<f32>;
}
```

## Correction Interface (src/correction/)

```rust
/// LLM text correction
pub trait TextCorrector: Send + Sync {
    fn correct(&self, text: &str) -> Result<String, CorrectionError>;
    fn is_available(&self) -> bool;
}
```

## Config Interface (src/config.rs)

```rust
impl Config {
    pub fn config_dir() -> Result<PathBuf, ConfigError>;
    pub fn data_dir() -> Result<PathBuf, ConfigError>;
    pub fn config_path() -> Result<PathBuf, ConfigError>;
    pub fn load() -> Result<Self, ConfigError>;
    pub fn save(&self) -> Result<(), ConfigError>;
}
```
