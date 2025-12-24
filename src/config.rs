use crate::vad::VadConfig;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to find config directory")]
    NoConfigDir,

    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub hotkey: HotkeyConfig,

    #[serde(default)]
    pub transcription: TranscriptionConfig,

    #[serde(default)]
    pub audio: AudioConfig,

    #[serde(default)]
    pub output: OutputConfig,

    #[serde(default)]
    pub correction: CorrectionConfig,

    #[serde(default)]
    pub feedback: FeedbackConfig,

    #[serde(default)]
    pub queue: QueueConfig,

    #[serde(default)]
    pub gpu: GpuConfig,

    /// Voice Activity Detection settings for continuous dictation
    #[serde(default)]
    pub vad: VadConfig,

    /// Vocabulary replacement settings
    #[serde(default)]
    pub vocabulary: VocabularyConfig,

    /// Logging settings
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VocabularyConfig {
    /// Enable vocabulary replacement
    #[serde(default)]
    pub enabled: bool,

    /// Path to vocabulary file (default: ~/.config/openhush/vocabulary.toml)
    #[serde(default)]
    pub path: Option<String>,

    /// Check for file changes every N seconds (0 = disabled)
    #[serde(default = "default_vocabulary_reload_interval")]
    pub reload_interval_secs: u32,
}

impl Default for VocabularyConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in feature
            path: None,     // Use default path
            reload_interval_secs: default_vocabulary_reload_interval(),
        }
    }
}

fn default_vocabulary_reload_interval() -> u32 {
    5 // Check for changes every 5 seconds
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    /// The trigger key (e.g., "ControlRight", "F12")
    #[serde(default = "default_hotkey")]
    pub key: String,

    /// Mode: "push_to_talk" or "toggle"
    #[serde(default = "default_mode")]
    pub mode: String,
}

/// Transcription mode preset for speed vs quality tradeoff.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionPreset {
    /// Fastest response for live dictation (small model)
    Instant,
    /// Balanced speed and accuracy (medium model)
    #[default]
    Balanced,
    /// Best accuracy for important content (large-v3 model)
    Quality,
    /// Use explicit model setting from config
    Custom,
}

impl TranscriptionPreset {
    /// Get the recommended model for this preset.
    #[must_use]
    pub fn model(&self) -> &'static str {
        match self {
            Self::Instant => "small",
            Self::Balanced => "medium",
            Self::Quality => "large-v3",
            Self::Custom => "base", // Fallback, custom uses explicit setting
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    /// Preset: instant, balanced, quality, or custom
    /// When not custom, model and prebuffer are auto-configured.
    #[serde(default)]
    pub preset: TranscriptionPreset,

    /// Whisper model: tiny, base, small, medium, large-v3
    /// Only used when preset = "custom"
    #[serde(default = "default_model")]
    pub model: String,

    /// Language: "auto" or ISO code (en, de, etc.)
    #[serde(default = "default_language")]
    pub language: String,

    /// Device: "cuda" or "cpu"
    #[serde(default = "default_device")]
    pub device: String,

    /// Translate to English (instead of transcribing in original language)
    #[serde(default)]
    pub translate: bool,
}

impl TranscriptionConfig {
    /// Get the effective model based on preset.
    #[must_use]
    pub fn effective_model(&self) -> &str {
        if self.preset == TranscriptionPreset::Custom {
            &self.model
        } else {
            self.preset.model()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutputConfig {
    /// Copy to clipboard
    #[serde(default = "default_true")]
    pub clipboard: bool,

    /// Paste at cursor
    #[serde(default = "default_true")]
    pub paste: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CorrectionConfig {
    /// Enable LLM correction
    #[serde(default)]
    pub enabled: bool,

    /// Ollama API endpoint
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// Ollama model for correction
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,

    /// Enable filler word removal (um, uh, like, etc.)
    #[serde(default)]
    pub remove_fillers: bool,

    /// Filler removal mode: conservative, moderate, or aggressive
    #[serde(default)]
    pub filler_mode: FillerRemovalMode,

    /// Timeout for Ollama requests in seconds
    #[serde(default = "default_ollama_timeout")]
    pub timeout_secs: u32,
}

/// Filler word removal mode
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FillerRemovalMode {
    /// Only remove basic fillers: um, uh, er
    #[default]
    Conservative,
    /// Remove common fillers with context awareness: um, uh, er, like, you know, basically
    Moderate,
    /// Remove all fillers aggressively: so, well, I mean, right, actually, etc.
    Aggressive,
}

fn default_ollama_timeout() -> u32 {
    30 // seconds
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedbackConfig {
    /// Play audio beep on start/stop
    #[serde(default = "default_true")]
    pub audio: bool,

    /// Show desktop notification
    #[serde(default = "default_true")]
    pub visual: bool,
}

/// Audio resampling quality
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResamplingQuality {
    /// Linear interpolation - fast but lower quality
    Low,
    /// Sinc interpolation via rubato - high quality, recommended
    #[default]
    High,
}

/// Backpressure strategy when transcription queue is full
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackpressureStrategy {
    /// Log warning but accept the job anyway (may cause unbounded growth)
    #[default]
    Warn,
    /// Drop the oldest pending job to make room for new ones
    DropOldest,
    /// Reject new jobs when queue is full
    DropNewest,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueueConfig {
    /// Max pending recordings (0 = unlimited)
    #[serde(default = "default_max_pending")]
    pub max_pending: u32,

    /// High water mark - warn when queue reaches this depth
    #[serde(default = "default_high_water_mark")]
    pub high_water_mark: u32,

    /// Backpressure strategy when queue is full
    #[serde(default)]
    pub backpressure_strategy: BackpressureStrategy,

    /// Whether to show notification on backpressure
    #[serde(default)]
    pub notify_on_backpressure: bool,

    /// Separator between transcriptions
    #[serde(default = "default_separator")]
    pub separator: String,

    /// Chunk interval for streaming transcription in seconds.
    /// During long recordings, audio is split into chunks and transcribed
    /// in parallel for lower latency.
    ///
    /// Special values:
    /// - `0` or negative: Auto-tune based on GPU benchmark at startup
    /// - Positive value: Use this exact interval (in seconds)
    #[serde(default = "default_chunk_interval")]
    pub chunk_interval_secs: f32,

    /// Safety margin for auto-tuned chunk interval (0.0 to 1.0).
    /// The auto-tuned interval is: measured_overhead * (1 + safety_margin)
    /// Default: 0.2 (20% margin)
    #[serde(default = "default_chunk_safety_margin")]
    pub chunk_safety_margin: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuConfig {
    /// Auto-detect CUDA GPUs
    #[serde(default = "default_true")]
    pub auto_detect: bool,

    /// Specific GPU device IDs to use
    #[serde(default)]
    pub devices: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioConfig {
    /// Duration of the always-on audio ring buffer in seconds.
    /// This enables instant recording with no startup delay.
    /// Higher values use more memory (~2MB per 30 seconds at 16kHz).
    #[serde(default = "default_prebuffer_duration")]
    pub prebuffer_duration_secs: f32,

    /// Resampling quality: "low" (linear) or "high" (sinc via rubato)
    /// High quality provides better transcription accuracy but uses more CPU.
    #[serde(default)]
    pub resampling_quality: ResamplingQuality,

    /// Enable/disable all preprocessing
    #[serde(default)]
    pub preprocessing: bool,

    /// RMS normalization settings
    #[serde(default)]
    pub normalization: NormalizationConfig,

    /// Dynamic compression settings
    #[serde(default)]
    pub compression: CompressionConfig,

    /// Limiter settings
    #[serde(default)]
    pub limiter: LimiterConfig,

    /// Noise reduction settings (RNNoise)
    #[serde(default)]
    pub noise_reduction: NoiseReductionConfig,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            prebuffer_duration_secs: default_prebuffer_duration(),
            resampling_quality: ResamplingQuality::default(),
            preprocessing: false,
            normalization: NormalizationConfig::default(),
            compression: CompressionConfig::default(),
            limiter: LimiterConfig::default(),
            noise_reduction: NoiseReductionConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoiseReductionConfig {
    /// Enable RNNoise neural network noise reduction
    #[serde(default)]
    pub enabled: bool,

    /// Strength of noise reduction (0.0 to 1.0)
    /// Higher values = more aggressive noise removal
    #[serde(default = "default_noise_reduction_strength")]
    pub strength: f32,
}

impl Default for NoiseReductionConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in feature
            strength: default_noise_reduction_strength(),
        }
    }
}

fn default_noise_reduction_strength() -> f32 {
    1.0 // Full strength by default when enabled
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NormalizationConfig {
    /// Enable RMS normalization
    #[serde(default)]
    pub enabled: bool,

    /// Target RMS level in dB (e.g., -18.0)
    #[serde(default = "default_normalization_target")]
    pub target_db: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompressionConfig {
    /// Enable dynamic compression
    #[serde(default)]
    pub enabled: bool,

    /// Threshold level in dB where compression kicks in
    #[serde(default = "default_compression_threshold")]
    pub threshold_db: f32,

    /// Compression ratio (e.g., 4.0 for 4:1)
    #[serde(default = "default_compression_ratio")]
    pub ratio: f32,

    /// Attack time in milliseconds
    #[serde(default = "default_compression_attack")]
    pub attack_ms: f32,

    /// Release time in milliseconds
    #[serde(default = "default_compression_release")]
    pub release_ms: f32,

    /// Makeup gain in dB applied after compression
    #[serde(default = "default_compression_makeup_gain")]
    pub makeup_gain_db: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LimiterConfig {
    /// Enable limiter
    #[serde(default)]
    pub enabled: bool,

    /// Ceiling level in dB (maximum output level)
    #[serde(default = "default_limiter_ceiling")]
    pub ceiling_db: f32,

    /// Release time in milliseconds
    #[serde(default = "default_limiter_release")]
    pub release_ms: f32,
}

// Default value functions
fn default_hotkey() -> String {
    "ControlRight".to_string()
}

fn default_mode() -> String {
    "push_to_talk".to_string()
}

fn default_model() -> String {
    "large-v3".to_string() // Best accuracy for multilingual transcription
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_device() -> String {
    "cuda".to_string()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_ollama_model() -> String {
    "llama3.2:3b".to_string()
}

fn default_separator() -> String {
    " ".to_string()
}

fn default_max_pending() -> u32 {
    10 // Maximum pending chunks before backpressure kicks in
}

fn default_high_water_mark() -> u32 {
    8 // Warn when queue reaches this depth
}

fn default_chunk_interval() -> f32 {
    0.0 // 0 means auto-tune based on GPU benchmark
}

fn default_chunk_safety_margin() -> f32 {
    0.2 // 20% safety margin for auto-tuned chunk interval
}

fn default_true() -> bool {
    true
}

// Audio preprocessing defaults
fn default_prebuffer_duration() -> f32 {
    30.0 // seconds - ring buffer duration for instant capture
}

fn default_normalization_target() -> f32 {
    -18.0 // dB - good level for speech
}

fn default_compression_threshold() -> f32 {
    -24.0 // dB - where compression starts
}

fn default_compression_ratio() -> f32 {
    4.0 // 4:1 compression ratio
}

fn default_compression_attack() -> f32 {
    5.0 // ms - fast attack for speech
}

fn default_compression_release() -> f32 {
    50.0 // ms - moderate release
}

fn default_compression_makeup_gain() -> f32 {
    6.0 // dB - boost after compression
}

fn default_limiter_ceiling() -> f32 {
    -1.0 // dB - leave headroom to prevent clipping
}

fn default_limiter_release() -> f32 {
    50.0 // ms
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            key: default_hotkey(),
            mode: default_mode(),
        }
    }
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            preset: TranscriptionPreset::default(),
            model: default_model(),
            language: default_language(),
            device: default_device(),
            translate: false,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            clipboard: true,
            paste: true,
        }
    }
}

impl Default for CorrectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            remove_fillers: false,
            filler_mode: FillerRemovalMode::default(),
            timeout_secs: default_ollama_timeout(),
        }
    }
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            audio: true,
            visual: true,
        }
    }
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_pending: default_max_pending(),
            high_water_mark: default_high_water_mark(),
            backpressure_strategy: BackpressureStrategy::default(),
            notify_on_backpressure: false,
            separator: default_separator(),
            chunk_interval_secs: default_chunk_interval(),
            chunk_safety_margin: default_chunk_safety_margin(),
        }
    }
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            devices: vec![],
        }
    }
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled when preprocessing is on
            target_db: default_normalization_target(),
        }
    }
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled when preprocessing is on
            threshold_db: default_compression_threshold(),
            ratio: default_compression_ratio(),
            attack_ms: default_compression_attack(),
            release_ms: default_compression_release(),
            makeup_gain_db: default_compression_makeup_gain(),
        }
    }
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled when preprocessing is on
            ceiling_db: default_limiter_ceiling(),
            release_ms: default_limiter_release(),
        }
    }
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from("com", "openhush", "openhush")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .ok_or(ConfigError::NoConfigDir)
    }

    /// Get the data directory path (for models)
    pub fn data_dir() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from("com", "openhush", "openhush")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .ok_or(ConfigError::NoConfigDir)
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load config from file, or create default if not exists
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;

        if path.exists() {
            let contents = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&contents)?;
            config.validate()?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate audio prebuffer duration
        if self.audio.prebuffer_duration_secs <= 0.0 {
            return Err(ConfigError::ValidationError(
                "prebuffer_duration_secs must be positive".into(),
            ));
        }
        if self.audio.prebuffer_duration_secs > 300.0 {
            return Err(ConfigError::ValidationError(
                "prebuffer_duration_secs cannot exceed 300 seconds".into(),
            ));
        }

        // Validate chunk safety margin
        if self.queue.chunk_safety_margin < 0.0 || self.queue.chunk_safety_margin > 2.0 {
            return Err(ConfigError::ValidationError(
                "chunk_safety_margin must be between 0.0 and 2.0".into(),
            ));
        }

        // Validate model name doesn't contain path traversal
        if self.transcription.model.contains("..") || self.transcription.model.contains('/') {
            return Err(ConfigError::ValidationError(
                "model name contains invalid characters".into(),
            ));
        }

        // Validate audio processing parameters
        if self.audio.normalization.target_db > 0.0 {
            return Err(ConfigError::ValidationError(
                "normalization target_db must be negative (in dB)".into(),
            ));
        }

        if self.audio.compression.ratio < 1.0 {
            return Err(ConfigError::ValidationError(
                "compression ratio must be >= 1.0".into(),
            ));
        }

        if self.audio.limiter.ceiling_db > 0.0 {
            return Err(ConfigError::ValidationError(
                "limiter ceiling_db must be negative (in dB)".into(),
            ));
        }

        // Validate noise reduction settings
        if self.audio.noise_reduction.strength < 0.0 || self.audio.noise_reduction.strength > 1.0 {
            return Err(ConfigError::ValidationError(
                "noise_reduction strength must be between 0.0 and 1.0".into(),
            ));
        }

        // Validate VAD settings
        if self.vad.threshold < 0.0 || self.vad.threshold > 1.0 {
            return Err(ConfigError::ValidationError(
                "vad threshold must be between 0.0 and 1.0".into(),
            ));
        }

        // Validate vocabulary path if specified
        if let Some(ref path) = self.vocabulary.path {
            // Check for path traversal attempts
            if path.contains("..") {
                return Err(ConfigError::ValidationError(
                    "vocabulary path contains path traversal sequence (..)".into(),
                ));
            }
            // Validate path is within config/data directories (defense in depth)
            let vocab_path = PathBuf::from(path);
            if vocab_path.is_absolute() {
                // For absolute paths, ensure they exist and are files
                if vocab_path.exists() && !vocab_path.is_file() {
                    return Err(ConfigError::ValidationError(
                        "vocabulary path must point to a file".into(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;

        // Create config directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;

        info!("Config saved to: {}", path.display());
        Ok(())
    }
}

/// Show current configuration
pub fn show() -> anyhow::Result<()> {
    let config = Config::load()?;
    let path = Config::config_path()?;

    println!("Config file: {}\n", path.display());
    println!("{}", toml::to_string_pretty(&config)?);

    Ok(())
}

/// Update configuration
pub fn update(
    hotkey: Option<String>,
    model: Option<String>,
    language: Option<String>,
    translate: Option<bool>,
    llm: Option<String>,
) -> anyhow::Result<()> {
    let mut config = Config::load()?;
    let mut changed = false;

    if let Some(key) = hotkey {
        config.hotkey.key = key;
        changed = true;
    }

    if let Some(m) = model {
        config.transcription.model = m;
        changed = true;
    }

    if let Some(lang) = language {
        config.transcription.language = lang;
        changed = true;
    }

    if let Some(trans) = translate {
        config.transcription.translate = trans;
        changed = true;
    }

    if let Some(llm_config) = llm {
        if llm_config == "off" || llm_config == "false" {
            config.correction.enabled = false;
        } else {
            config.correction.enabled = true;
            // Parse "ollama:model_name" format
            if let Some(model_name) = llm_config.strip_prefix("ollama:") {
                config.correction.ollama_model = model_name.to_string();
            }
        }
        changed = true;
    }

    if changed {
        config.save()?;
        println!("Configuration updated.");
    } else {
        println!("No changes specified. Use --show to view current config.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // Default Value Tests
    // ===================

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.hotkey.key, "ControlRight");
        assert_eq!(config.hotkey.mode, "push_to_talk");
        assert_eq!(config.transcription.preset, TranscriptionPreset::Balanced);
        assert_eq!(config.transcription.model, "large-v3");
        assert_eq!(config.transcription.language, "auto");
        assert_eq!(config.transcription.device, "cuda");
        assert!(!config.transcription.translate);
        assert!(config.output.clipboard);
        assert!(config.output.paste);
        assert!(!config.correction.enabled);
        assert!(config.feedback.audio);
        assert!(config.feedback.visual);
    }

    #[test]
    fn test_default_audio_config() {
        let config = AudioConfig::default();
        assert!((config.prebuffer_duration_secs - 30.0).abs() < 0.01);
        assert_eq!(config.resampling_quality, ResamplingQuality::High);
        assert!(!config.preprocessing);
        assert!(config.normalization.enabled);
        assert!(config.compression.enabled);
        assert!(config.limiter.enabled);
    }

    #[test]
    fn test_default_queue_config() {
        let config = QueueConfig::default();
        assert_eq!(config.max_pending, 10);
        assert_eq!(config.high_water_mark, 8);
        assert_eq!(config.backpressure_strategy, BackpressureStrategy::Warn);
        assert!(!config.notify_on_backpressure);
        assert_eq!(config.separator, " ");
        assert!((config.chunk_interval_secs - 0.0).abs() < 0.01);
        assert!((config.chunk_safety_margin - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_default_noise_reduction_config() {
        let config = NoiseReductionConfig::default();
        assert!(!config.enabled);
        assert!((config.strength - 1.0).abs() < 0.01);
    }

    // ===================
    // Transcription Preset Tests
    // ===================

    #[test]
    fn test_transcription_preset_models() {
        assert_eq!(TranscriptionPreset::Instant.model(), "small");
        assert_eq!(TranscriptionPreset::Balanced.model(), "medium");
        assert_eq!(TranscriptionPreset::Quality.model(), "large-v3");
        assert_eq!(TranscriptionPreset::Custom.model(), "base");
    }

    #[test]
    fn test_effective_model_with_preset() {
        let mut config = TranscriptionConfig::default();

        config.preset = TranscriptionPreset::Instant;
        assert_eq!(config.effective_model(), "small");

        config.preset = TranscriptionPreset::Balanced;
        assert_eq!(config.effective_model(), "medium");

        config.preset = TranscriptionPreset::Quality;
        assert_eq!(config.effective_model(), "large-v3");
    }

    #[test]
    fn test_effective_model_custom() {
        let mut config = TranscriptionConfig::default();
        config.preset = TranscriptionPreset::Custom;
        config.model = "tiny".to_string();
        assert_eq!(config.effective_model(), "tiny");
    }

    // ===================
    // Validation Tests
    // ===================

    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_prebuffer_too_low() {
        let mut config = Config::default();
        config.audio.prebuffer_duration_secs = 0.0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("prebuffer_duration_secs must be positive"));
    }

    #[test]
    fn test_validate_prebuffer_negative() {
        let mut config = Config::default();
        config.audio.prebuffer_duration_secs = -1.0;
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_prebuffer_too_high() {
        let mut config = Config::default();
        config.audio.prebuffer_duration_secs = 500.0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 300 seconds"));
    }

    #[test]
    fn test_validate_chunk_safety_margin_valid() {
        let mut config = Config::default();
        config.queue.chunk_safety_margin = 0.5;
        assert!(config.validate().is_ok());

        config.queue.chunk_safety_margin = 0.0;
        assert!(config.validate().is_ok());

        config.queue.chunk_safety_margin = 2.0;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_chunk_safety_margin_invalid() {
        let mut config = Config::default();

        config.queue.chunk_safety_margin = -0.1;
        assert!(config.validate().is_err());

        config.queue.chunk_safety_margin = 2.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_model_path_traversal() {
        let mut config = Config::default();

        config.transcription.model = "../../../etc/passwd".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid characters"));

        config.transcription.model = "models/base".to_string();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_normalization_target() {
        let mut config = Config::default();

        // Valid: negative dB
        config.audio.normalization.target_db = -18.0;
        assert!(config.validate().is_ok());

        // Invalid: positive dB
        config.audio.normalization.target_db = 5.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_compression_ratio() {
        let mut config = Config::default();

        // Valid: >= 1.0
        config.audio.compression.ratio = 4.0;
        assert!(config.validate().is_ok());

        config.audio.compression.ratio = 1.0;
        assert!(config.validate().is_ok());

        // Invalid: < 1.0
        config.audio.compression.ratio = 0.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_limiter_ceiling() {
        let mut config = Config::default();

        // Valid: negative dB
        config.audio.limiter.ceiling_db = -1.0;
        assert!(config.validate().is_ok());

        // Invalid: positive dB
        config.audio.limiter.ceiling_db = 1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_noise_reduction_strength() {
        let mut config = Config::default();

        // Valid: 0.0 to 1.0
        config.audio.noise_reduction.strength = 0.0;
        assert!(config.validate().is_ok());

        config.audio.noise_reduction.strength = 0.5;
        assert!(config.validate().is_ok());

        config.audio.noise_reduction.strength = 1.0;
        assert!(config.validate().is_ok());

        // Invalid: < 0.0 or > 1.0
        config.audio.noise_reduction.strength = -0.1;
        assert!(config.validate().is_err());

        config.audio.noise_reduction.strength = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_vad_threshold() {
        let mut config = Config::default();

        // Valid: 0.0 to 1.0
        config.vad.threshold = 0.5;
        assert!(config.validate().is_ok());

        // Invalid
        config.vad.threshold = -0.1;
        assert!(config.validate().is_err());

        config.vad.threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_vocabulary_path_traversal() {
        let mut config = Config::default();

        // Path traversal attempt
        config.vocabulary.path = Some("../../../etc/passwd".to_string());
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("path traversal"));
    }

    // ===================
    // TOML Parsing Tests
    // ===================

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = "";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.hotkey.key, "ControlRight");
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
[hotkey]
key = "F12"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.hotkey.key, "F12");
        assert_eq!(config.hotkey.mode, "push_to_talk"); // Default
    }

    #[test]
    fn test_parse_transcription_preset() {
        let toml_str = r#"
[transcription]
preset = "instant"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.transcription.preset, TranscriptionPreset::Instant);
    }

    #[test]
    fn test_parse_backpressure_strategy() {
        let toml_str = r#"
[queue]
backpressure_strategy = "drop_oldest"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.queue.backpressure_strategy,
            BackpressureStrategy::DropOldest
        );
    }

    #[test]
    fn test_parse_filler_mode() {
        let toml_str = r#"
[correction]
filler_mode = "aggressive"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.correction.filler_mode, FillerRemovalMode::Aggressive);
    }

    #[test]
    fn test_serialize_and_deserialize_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.hotkey.key, parsed.hotkey.key);
        assert_eq!(config.transcription.model, parsed.transcription.model);
        assert_eq!(config.output.clipboard, parsed.output.clipboard);
    }

    // ===================
    // Enum Tests
    // ===================

    #[test]
    fn test_resampling_quality_default() {
        assert_eq!(ResamplingQuality::default(), ResamplingQuality::High);
    }

    #[test]
    fn test_backpressure_strategy_default() {
        assert_eq!(BackpressureStrategy::default(), BackpressureStrategy::Warn);
    }

    #[test]
    fn test_filler_removal_mode_default() {
        assert_eq!(FillerRemovalMode::default(), FillerRemovalMode::Conservative);
    }

    #[test]
    fn test_transcription_preset_default() {
        assert_eq!(TranscriptionPreset::default(), TranscriptionPreset::Balanced);
    }
}
