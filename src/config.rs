use crate::output::actions::ActionConfig;
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

/// Channel selection for audio input.
///
/// Specifies which channels to capture from multi-channel audio sources.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ChannelSelection {
    /// Capture all channels and mix to mono (default)
    #[default]
    All,
    /// Capture only the specified channels (0-indexed) and mix to mono
    Select(Vec<u8>),
}

impl Serialize for ChannelSelection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ChannelSelection::All => serializer.serialize_str("all"),
            ChannelSelection::Select(channels) => channels.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ChannelSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct ChannelSelectionVisitor;

        impl<'de> Visitor<'de> for ChannelSelectionVisitor {
            type Value = ChannelSelection;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("\"all\" or an array of channel indices")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.eq_ignore_ascii_case("all") {
                    Ok(ChannelSelection::All)
                } else {
                    Err(de::Error::custom(format!(
                        "expected \"all\" or array, got \"{}\"",
                        value
                    )))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut channels = Vec::new();
                while let Some(ch) = seq.next_element()? {
                    channels.push(ch);
                }
                if channels.is_empty() {
                    Ok(ChannelSelection::All)
                } else {
                    Ok(ChannelSelection::Select(channels))
                }
            }
        }

        deserializer.deserialize_any(ChannelSelectionVisitor)
    }
}

#[allow(dead_code)]
impl ChannelSelection {
    /// Parse from a comma-separated string (for CLI)
    pub fn from_cli_arg(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("all") || s.is_empty() {
            return Ok(Self::All);
        }

        let channels: Result<Vec<u8>, _> = s
            .split(',')
            .map(|part| {
                part.trim()
                    .parse::<u8>()
                    .map_err(|_| format!("Invalid channel number: {}", part))
            })
            .collect();

        channels.map(Self::Select)
    }

    /// Get channel indices, or None for all channels
    pub fn indices(&self) -> Option<&[u8]> {
        match self {
            ChannelSelection::All => None,
            ChannelSelection::Select(channels) => Some(channels),
        }
    }
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

    /// Translation settings for multilingual output
    #[serde(default)]
    pub translation: TranslationConfig,

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

    /// Appearance settings (theme)
    #[serde(default)]
    pub appearance: AppearanceConfig,

    /// App-specific profiles for context-aware settings
    #[serde(default)]
    pub profiles: Vec<AppProfile>,

    /// Speaker diarization settings
    #[serde(default)]
    pub diarization: DiarizationConfig,

    /// Wake word detection settings ("Hey OpenHush")
    #[serde(default)]
    pub wake_word: WakeWordConfig,

    /// REST API server settings
    #[serde(default)]
    pub api: ApiConfig,

    /// Meeting summarization settings
    #[serde(default)]
    pub summarization: SummarizationConfig,
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

/// App-specific profile for context-aware settings.
///
/// Allows different settings per application (e.g., aggressive filler
/// removal in email clients, conservative mode in code editors).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppProfile {
    /// Profile name (for display/logging)
    pub name: String,

    /// Application names/classes to match (case-insensitive, partial match)
    #[serde(default)]
    pub apps: Vec<String>,

    /// Whether OpenHush is enabled for these apps
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Override vocabulary file for this profile
    #[serde(default)]
    pub vocabulary_file: Option<String>,

    /// Override snippets file for this profile
    #[serde(default)]
    pub snippets_file: Option<String>,

    /// Override filler removal level: "off", "conservative", "moderate", "aggressive"
    #[serde(default)]
    pub filler_removal: Option<String>,

    /// Override transcription preset: "instant", "balanced", "quality"
    #[serde(default)]
    pub preset: Option<String>,
}

impl AppProfile {
    /// Check if this profile matches the given app context.
    #[allow(dead_code)]
    pub fn matches(&self, app_name: &str) -> bool {
        let app_lower = app_name.to_lowercase();
        self.apps.iter().any(|pattern| {
            let pattern_lower = pattern.to_lowercase();
            app_lower == pattern_lower
                || app_lower.contains(&pattern_lower)
                || pattern_lower.contains(&app_lower)
        })
    }
}

/// Theme setting for the UI
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Light theme
    Light,
    /// Dark theme
    Dark,
    /// Follow system preference (using dark-light crate)
    #[default]
    Auto,
}

impl Theme {
    /// Detect the effective theme based on system preference.
    /// Returns true if dark mode should be used.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        match self {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::Auto => {
                // Use dark-light crate to detect system theme
                match dark_light::detect() {
                    dark_light::Mode::Dark => true,
                    dark_light::Mode::Light | dark_light::Mode::Default => false,
                }
            }
        }
    }

    /// Get display name for the theme
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::Auto => "System",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppearanceConfig {
    /// Theme: light, dark, or auto (follow system)
    #[serde(default)]
    pub theme: Theme,
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

/// Speaker diarization configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiarizationConfig {
    /// Enable speaker diarization in record mode
    #[serde(default)]
    pub enabled: bool,

    /// Maximum number of speakers to detect (1-10)
    #[serde(default = "default_max_speakers")]
    pub max_speakers: usize,

    /// Similarity threshold for speaker matching (0.0 - 1.0)
    /// Higher = stricter matching, more speakers detected
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,
}

impl Default for DiarizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_speakers: default_max_speakers(),
            similarity_threshold: default_similarity_threshold(),
        }
    }
}

fn default_max_speakers() -> usize {
    6
}

fn default_similarity_threshold() -> f32 {
    0.5
}

/// Wake word detection configuration ("Hey OpenHush").
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WakeWordConfig {
    /// Enable wake word detection (requires always-on listening)
    #[serde(default)]
    pub enabled: bool,

    /// Path to wake word model file (.rpw)
    /// If not set, uses default "hey open hush" model
    #[serde(default)]
    pub model_path: Option<String>,

    /// Detection sensitivity (0.0 = strict, 1.0 = loose)
    #[serde(default = "default_wake_word_sensitivity")]
    pub sensitivity: f32,

    /// Minimum score threshold for detection (0.0 - 1.0)
    #[serde(default = "default_wake_word_threshold")]
    pub threshold: f32,

    /// Seconds to record after wake word detected (0 = until silence)
    #[serde(default = "default_wake_word_timeout")]
    pub timeout_secs: f32,

    /// Enable audio feedback beep on wake word detection
    #[serde(default = "default_true")]
    pub beep_on_detect: bool,

    /// Enable visual notification on wake word detection
    #[serde(default = "default_true")]
    pub notify_on_detect: bool,
}

impl Default for WakeWordConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in (always-on microphone has privacy implications)
            model_path: None,
            sensitivity: default_wake_word_sensitivity(),
            threshold: default_wake_word_threshold(),
            timeout_secs: default_wake_word_timeout(),
            beep_on_detect: true,
            notify_on_detect: true,
        }
    }
}

fn default_wake_word_sensitivity() -> f32 {
    0.5 // Balanced sensitivity
}

fn default_wake_word_threshold() -> f32 {
    0.5 // Minimum detection confidence
}

fn default_wake_word_timeout() -> f32 {
    10.0 // 10 seconds max command length
}

/// REST API server configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiConfig {
    /// Enable REST API server (disabled by default for security)
    #[serde(default)]
    pub enabled: bool,

    /// Bind address (default: 127.0.0.1:8080 - localhost only)
    #[serde(default = "default_api_bind")]
    pub bind: String,

    /// API key hash (SHA-256) for authentication.
    /// Generate with: `openhush api-key generate`
    #[serde(default)]
    pub api_key_hash: Option<String>,

    /// Enable Swagger UI at /swagger-ui/
    #[serde(default = "default_true")]
    pub swagger_ui: bool,

    /// Allowed CORS origins (empty = same-origin only)
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for security
            bind: default_api_bind(),
            api_key_hash: None,
            swagger_ui: true,
            cors_origins: vec![],
        }
    }
}

fn default_api_bind() -> String {
    "127.0.0.1:8080".to_string()
}

/// Meeting summarization configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SummarizationConfig {
    /// Enable summarization feature
    #[serde(default)]
    pub enabled: bool,

    /// Default LLM provider: "ollama" or "openai"
    #[serde(default = "default_summarization_provider")]
    pub default_provider: String,

    /// Default template name (standup, meeting, retro, 1on1, summary)
    #[serde(default = "default_summarization_template")]
    pub default_template: String,

    /// Path to custom templates file (optional)
    #[serde(default)]
    pub templates_path: Option<String>,

    /// Ollama-specific settings
    #[serde(default)]
    pub ollama: SummarizationOllamaConfig,

    /// OpenAI-compatible API settings
    #[serde(default)]
    pub openai: SummarizationOpenAiConfig,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_provider: default_summarization_provider(),
            default_template: default_summarization_template(),
            templates_path: None,
            ollama: SummarizationOllamaConfig::default(),
            openai: SummarizationOpenAiConfig::default(),
        }
    }
}

fn default_summarization_provider() -> String {
    "ollama".to_string()
}

fn default_summarization_template() -> String {
    "meeting".to_string()
}

fn default_summarization_timeout() -> u32 {
    120 // 2 minutes for long transcripts
}

/// Ollama settings for summarization
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SummarizationOllamaConfig {
    /// Ollama API URL
    #[serde(default = "default_ollama_url")]
    pub url: String,

    /// Model name for summarization
    #[serde(default = "default_summarization_ollama_model")]
    pub model: String,

    /// Request timeout in seconds
    #[serde(default = "default_summarization_timeout")]
    pub timeout_secs: u32,
}

impl Default for SummarizationOllamaConfig {
    fn default() -> Self {
        Self {
            url: default_ollama_url(),
            model: default_summarization_ollama_model(),
            timeout_secs: default_summarization_timeout(),
        }
    }
}

fn default_summarization_ollama_model() -> String {
    "llama3.2:3b".to_string()
}

/// OpenAI-compatible API settings for summarization
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SummarizationOpenAiConfig {
    /// API key (supports "keyring:" prefix for secure storage)
    #[serde(default)]
    pub api_key: String,

    /// Model name
    #[serde(default = "default_openai_model")]
    pub model: String,

    /// Base URL (override for compatible APIs like Groq, Claude, etc.)
    #[serde(default = "default_openai_base_url")]
    pub base_url: String,

    /// Request timeout in seconds
    #[serde(default = "default_summarization_timeout")]
    pub timeout_secs: u32,
}

impl Default for SummarizationOpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openai_model(),
            base_url: default_openai_base_url(),
            timeout_secs: default_summarization_timeout(),
        }
    }
}

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
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

    /// Unload model after N seconds of inactivity (0 = never unload).
    /// Useful for freeing GPU memory when not in use.
    #[serde(default)]
    pub idle_unload_secs: u32,

    /// Pre-load model on daemon start.
    /// If false, model is loaded lazily on first transcription request.
    #[serde(default = "default_true")]
    pub preload: bool,
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

    /// Post-transcription actions (shell commands, HTTP requests, file logging)
    #[serde(default)]
    pub actions: Vec<ActionConfig>,
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

/// Translation engine type
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranslationEngine {
    /// M2M-100 neural translation (local, high quality)
    #[default]
    M2m100,
    /// Ollama LLM-based translation (local, flexible)
    Ollama,
}

/// Translation configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationConfig {
    /// Enable translation of transcriptions
    #[serde(default)]
    pub enabled: bool,

    /// Translation engine to use
    #[serde(default)]
    pub engine: TranslationEngine,

    /// Target language code (e.g., "en", "de", "fr")
    #[serde(default = "default_target_language")]
    pub target_language: String,

    /// Preserve original text alongside translation
    #[serde(default)]
    pub preserve_original: bool,

    /// M2M-100 model variant: "418m" (small) or "1.2b" (large)
    #[serde(default = "default_m2m100_model")]
    pub m2m100_model: String,

    /// Ollama API endpoint (if engine = ollama)
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// Ollama model for translation (if engine = ollama)
    #[serde(default = "default_translation_ollama_model")]
    pub ollama_model: String,

    /// Timeout for translation requests in seconds
    #[serde(default = "default_translation_timeout")]
    pub timeout_secs: u32,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: TranslationEngine::default(),
            target_language: default_target_language(),
            preserve_original: false,
            m2m100_model: default_m2m100_model(),
            ollama_url: default_ollama_url(),
            ollama_model: default_translation_ollama_model(),
            timeout_secs: default_translation_timeout(),
        }
    }
}

fn default_target_language() -> String {
    "en".to_string()
}

fn default_m2m100_model() -> String {
    "418m".to_string()
}

fn default_translation_ollama_model() -> String {
    "llama3.2:3b".to_string()
}

fn default_translation_timeout() -> u32 {
    60 // seconds - translation can take longer than correction
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

    /// Channel selection for multi-channel audio sources.
    /// - "all": capture all channels and mix to mono (default)
    /// - \[0\]: capture only channel 0
    /// - \[0, 1\]: capture channels 0 and 1, mix to mono
    #[serde(default)]
    pub channels: ChannelSelection,

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
            channels: ChannelSelection::default(),
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
            idle_unload_secs: 0,
            preload: true,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            clipboard: true,
            paste: true,
            actions: vec![],
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

    /// Find the matching profile for an app name.
    ///
    /// Returns the first matching profile, or None if no profile matches.
    #[allow(dead_code)]
    pub fn find_profile(&self, app_name: &str) -> Option<&AppProfile> {
        self.profiles.iter().find(|p| p.matches(app_name))
    }

    /// Check if OpenHush is enabled for the given app.
    ///
    /// Returns true if no profile matches (default enabled) or if the
    /// matching profile has enabled=true.
    #[allow(dead_code)]
    pub fn is_enabled_for_app(&self, app_name: &str) -> bool {
        match self.find_profile(app_name) {
            Some(profile) => profile.enabled,
            None => true, // Default enabled
        }
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
#[allow(clippy::too_many_arguments)]
pub fn update(
    hotkey: Option<String>,
    model: Option<String>,
    language: Option<String>,
    translate: Option<bool>,
    llm: Option<String>,
    translation: Option<bool>,
    translation_engine: Option<String>,
    translation_target: Option<String>,
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

    // Translation settings
    if let Some(enabled) = translation {
        config.translation.enabled = enabled;
        changed = true;
    }

    if let Some(engine) = translation_engine {
        let engine_lower = engine.to_lowercase();
        match engine_lower.as_str() {
            "m2m100" | "m2m" => config.translation.engine = TranslationEngine::M2m100,
            "ollama" | "llm" => config.translation.engine = TranslationEngine::Ollama,
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown translation engine '{}'. Use 'm2m100' or 'ollama'.",
                    engine
                ));
            }
        }
        changed = true;
    }

    if let Some(target) = translation_target {
        config.translation.target_language = target;
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
        assert_eq!(config.transcription.idle_unload_secs, 0);
        assert!(config.transcription.preload);
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
        let config = TranscriptionConfig {
            preset: TranscriptionPreset::Instant,
            ..Default::default()
        };
        assert_eq!(config.effective_model(), "small");

        let config = TranscriptionConfig {
            preset: TranscriptionPreset::Balanced,
            ..Default::default()
        };
        assert_eq!(config.effective_model(), "medium");

        let config = TranscriptionConfig {
            preset: TranscriptionPreset::Quality,
            ..Default::default()
        };
        assert_eq!(config.effective_model(), "large-v3");
    }

    #[test]
    fn test_effective_model_custom() {
        let config = TranscriptionConfig {
            preset: TranscriptionPreset::Custom,
            model: "tiny".to_string(),
            ..Default::default()
        };
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
        assert!(result.unwrap_err().to_string().contains("path traversal"));
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
        assert_eq!(
            FillerRemovalMode::default(),
            FillerRemovalMode::Conservative
        );
    }

    #[test]
    fn test_transcription_preset_default() {
        assert_eq!(
            TranscriptionPreset::default(),
            TranscriptionPreset::Balanced
        );
    }

    // ===================
    // Theme Tests
    // ===================

    #[test]
    fn test_theme_default() {
        assert_eq!(Theme::default(), Theme::Auto);
    }

    #[test]
    fn test_theme_is_dark() {
        assert!(!Theme::Light.is_dark());
        assert!(Theme::Dark.is_dark());
        // Auto depends on system, so we just verify it returns a bool
        let _ = Theme::Auto.is_dark();
    }

    #[test]
    fn test_theme_display_name() {
        assert_eq!(Theme::Light.display_name(), "Light");
        assert_eq!(Theme::Dark.display_name(), "Dark");
        assert_eq!(Theme::Auto.display_name(), "System");
    }

    #[test]
    fn test_parse_theme_from_toml() {
        let toml_str = r#"
[appearance]
theme = "dark"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.appearance.theme, Theme::Dark);

        let toml_str = r#"
[appearance]
theme = "light"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.appearance.theme, Theme::Light);

        let toml_str = r#"
[appearance]
theme = "auto"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.appearance.theme, Theme::Auto);
    }

    #[test]
    fn test_appearance_config_default() {
        let config = AppearanceConfig::default();
        assert_eq!(config.theme, Theme::Auto);
    }

    // ===================
    // ChannelSelection Tests
    // ===================

    #[test]
    fn test_channel_selection_default() {
        assert_eq!(ChannelSelection::default(), ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_serialize_all() {
        let sel = ChannelSelection::All;
        let json = serde_json::to_string(&sel).unwrap();
        assert_eq!(json, "\"all\"");
    }

    #[test]
    fn test_channel_selection_serialize_select() {
        let sel = ChannelSelection::Select(vec![0, 2, 4]);
        let json = serde_json::to_string(&sel).unwrap();
        assert_eq!(json, "[0,2,4]");
    }

    #[test]
    fn test_channel_selection_deserialize_all_string() {
        let sel: ChannelSelection = serde_json::from_str("\"all\"").unwrap();
        assert_eq!(sel, ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_deserialize_all_uppercase() {
        let sel: ChannelSelection = serde_json::from_str("\"ALL\"").unwrap();
        assert_eq!(sel, ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_deserialize_array() {
        let sel: ChannelSelection = serde_json::from_str("[1, 3, 5]").unwrap();
        assert_eq!(sel, ChannelSelection::Select(vec![1, 3, 5]));
    }

    #[test]
    fn test_channel_selection_deserialize_empty_array() {
        let sel: ChannelSelection = serde_json::from_str("[]").unwrap();
        assert_eq!(sel, ChannelSelection::All); // Empty array becomes All
    }

    #[test]
    fn test_channel_selection_from_cli_arg_all() {
        let sel = ChannelSelection::from_cli_arg("all").unwrap();
        assert_eq!(sel, ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_from_cli_arg_all_uppercase() {
        let sel = ChannelSelection::from_cli_arg("ALL").unwrap();
        assert_eq!(sel, ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_from_cli_arg_empty() {
        let sel = ChannelSelection::from_cli_arg("").unwrap();
        assert_eq!(sel, ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_from_cli_arg_channels() {
        let sel = ChannelSelection::from_cli_arg("0,1,2").unwrap();
        assert_eq!(sel, ChannelSelection::Select(vec![0, 1, 2]));
    }

    #[test]
    fn test_channel_selection_from_cli_arg_with_spaces() {
        let sel = ChannelSelection::from_cli_arg("0, 1, 2").unwrap();
        assert_eq!(sel, ChannelSelection::Select(vec![0, 1, 2]));
    }

    #[test]
    fn test_channel_selection_from_cli_arg_invalid() {
        let result = ChannelSelection::from_cli_arg("0,abc,2");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("abc"));
    }

    #[test]
    fn test_channel_selection_indices_all() {
        let sel = ChannelSelection::All;
        assert_eq!(sel.indices(), None);
    }

    #[test]
    fn test_channel_selection_indices_select() {
        let sel = ChannelSelection::Select(vec![0, 2]);
        assert_eq!(sel.indices(), Some(&[0, 2][..]));
    }

    #[test]
    fn test_channel_selection_toml_all() {
        let toml_str = r#"
[audio]
channels = "all"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.audio.channels, ChannelSelection::All);
    }

    #[test]
    fn test_channel_selection_toml_select() {
        let toml_str = r#"
[audio]
channels = [0, 1]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.audio.channels, ChannelSelection::Select(vec![0, 1]));
    }

    #[test]
    fn test_channel_selection_roundtrip() {
        let original = ChannelSelection::Select(vec![1, 3, 5]);
        let json = serde_json::to_string(&original).unwrap();
        let parsed: ChannelSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}
