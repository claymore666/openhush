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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    /// Whisper model: tiny, base, small, medium, large-v3
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueueConfig {
    /// Max pending recordings (0 = unlimited)
    #[serde(default)]
    pub max_pending: u32,

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
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            prebuffer_duration_secs: default_prebuffer_duration(),
            preprocessing: false,
            normalization: NormalizationConfig::default(),
            compression: CompressionConfig::default(),
            limiter: LimiterConfig::default(),
        }
    }
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
            max_pending: 0,
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
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
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
