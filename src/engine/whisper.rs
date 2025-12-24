//! Whisper transcription engine using whisper-rs.

use crate::config::Config;
use crate::engine::validation::{self, AudioValidationError};
use crate::input::AudioBuffer;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

#[derive(Error, Debug)]
pub enum WhisperError {
    #[error("Model not found at {0}. Run 'openhush model download {1}'")]
    ModelNotFound(PathBuf, String),

    #[error("Failed to load model: {0}")]
    LoadFailed(String),

    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),

    #[error("Audio validation failed: {0}")]
    ValidationFailed(#[from] AudioValidationError),
}

/// Result of transcription
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Language detected or used
    #[allow(dead_code)]
    pub language: String,
    /// Processing time in milliseconds
    #[allow(dead_code)]
    pub duration_ms: u64,
}

/// Available Whisper models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    LargeV3,
}

impl std::str::FromStr for WhisperModel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tiny" => Ok(Self::Tiny),
            "base" => Ok(Self::Base),
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "large" | "large-v3" | "largev3" => Ok(Self::LargeV3),
            _ => Err(()),
        }
    }
}

impl WhisperModel {
    /// Get the model filename
    #[must_use]
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::Base => "ggml-base.bin",
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
            Self::LargeV3 => "ggml-large-v3.bin",
        }
    }

    /// Get model size in bytes (approximate)
    #[must_use]
    #[allow(dead_code)]
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Tiny => 75_000_000,
            Self::Base => 142_000_000,
            Self::Small => 466_000_000,
            Self::Medium => 1_500_000_000,
            Self::LargeV3 => 3_000_000_000,
        }
    }

    /// Get Hugging Face download URL
    #[allow(dead_code)]
    pub fn download_url(&self) -> String {
        let filename = self.filename();
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            filename
        )
    }
}

/// Whisper transcription engine with cached state for fast inference
pub struct WhisperEngine {
    /// Cached state for reuse across transcriptions (avoids GPU buffer reallocation)
    state: RefCell<WhisperState>,
    language: String,
    translate: bool,
}

impl WhisperEngine {
    /// Create a new Whisper engine, loading the model from disk
    ///
    /// If `translate` is true, all audio will be translated to English.
    /// If false, audio will be transcribed in its original language.
    ///
    /// The engine pre-allocates GPU buffers for fast transcription.
    pub fn new(
        model_path: &Path,
        language: &str,
        translate: bool,
        use_gpu: bool,
    ) -> Result<Self, WhisperError> {
        info!(
            "Loading Whisper model from: {} (GPU: {})",
            model_path.display(),
            use_gpu
        );

        if !model_path.exists() {
            // Extract model name from path for error message
            let model_name = model_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .strip_prefix("ggml-")
                .unwrap_or("unknown");

            return Err(WhisperError::ModelNotFound(
                model_path.to_path_buf(),
                model_name.to_string(),
            ));
        }

        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu);

        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap_or_default(), params)
            .map_err(|e| WhisperError::LoadFailed(format!("{:?}", e)))?;

        // Leak the context so the state can reference it for the daemon's lifetime
        let ctx: &'static WhisperContext = Box::leak(Box::new(ctx));

        // Pre-create state to allocate GPU buffers once
        info!("Pre-allocating GPU buffers...");
        let state = ctx
            .create_state()
            .map_err(|e| WhisperError::LoadFailed(format!("Failed to create state: {:?}", e)))?;

        info!("Whisper model loaded and GPU buffers allocated");

        Ok(Self {
            state: RefCell::new(state),
            language: language.to_string(),
            translate,
        })
    }

    /// Load engine from config
    #[allow(dead_code)]
    pub fn from_config(config: &Config) -> Result<Self, WhisperError> {
        let data_dir = Config::data_dir().map_err(|e| WhisperError::LoadFailed(e.to_string()))?;

        let model = config
            .transcription
            .effective_model()
            .parse()
            .unwrap_or(WhisperModel::Base);

        let model_path = data_dir.join("models").join(model.filename());
        let use_gpu = config.transcription.device.to_lowercase() != "cpu";

        Self::new(
            &model_path,
            &config.transcription.language,
            config.transcription.translate,
            use_gpu,
        )
    }

    /// Transcribe audio buffer to text
    pub fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult, WhisperError> {
        // Validate audio before FFI boundary
        let validation_info = validation::validate_audio(&audio.samples, audio.sample_rate)?;

        debug!(
            "Audio validated: {:.2}s, {} samples, RMS: {:.4}, range: [{:.3}, {:.3}]",
            validation_info.duration_secs,
            validation_info.sample_count,
            validation_info.rms,
            validation_info.min_value,
            validation_info.max_value
        );

        // Warn if audio levels seem unusual
        if validation_info.rms < 0.001 {
            warn!(
                "Audio appears to be silence or very quiet (RMS: {:.6})",
                validation_info.rms
            );
        }
        if validation_info.max_value.abs() > 1.0 || validation_info.min_value.abs() > 1.0 {
            warn!(
                "Audio samples outside normal range [-1, 1]: min={:.3}, max={:.3}",
                validation_info.min_value, validation_info.max_value
            );
        }

        let start_time = std::time::Instant::now();

        debug!(
            "Transcribing {:.2}s of audio ({} samples)",
            audio.duration_secs(),
            audio.samples.len()
        );

        // Use cached state (GPU buffers already allocated)
        let mut state = self.state.borrow_mut();

        // Configure parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language
        if self.language != "auto" {
            params.set_language(Some(&self.language));
        }

        // Set translate mode (true = translate to English, false = transcribe in original language)
        debug!("Setting translate={}", self.translate);
        params.set_translate(self.translate);

        // Disable printing to avoid cluttering output
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Run inference
        state
            .full(params, &audio.samples)
            .map_err(|e| WhisperError::TranscriptionFailed(format!("{:?}", e)))?;

        // Collect results
        let num_segments = state
            .full_n_segments()
            .map_err(|e| WhisperError::TranscriptionFailed(format!("{:?}", e)))?;

        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }

        // Trim whitespace
        let text = text.trim().to_string();

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Get detected/used language
        let detected_lang = if self.language == "auto" {
            // Get detected language ID from whisper state
            match state.full_lang_id_from_state() {
                Ok(lang_id) => lang_id_to_code(lang_id).to_string(),
                Err(_) => "auto".to_string(),
            }
        } else {
            self.language.clone()
        };

        info!(
            "Transcription complete ({} chars, {}ms, lang: {})",
            text.len(),
            duration_ms,
            detected_lang
        );

        Ok(TranscriptionResult {
            text,
            language: detected_lang,
            duration_ms,
        })
    }
}

/// Result of GPU benchmark
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Fixed overhead in seconds (time to process ~2s of audio)
    pub overhead_secs: f32,
    /// Recommended minimum chunk interval in seconds
    pub recommended_chunk_interval: f32,
    /// Audio duration used for benchmark
    #[allow(dead_code)]
    pub test_audio_secs: f32,
}

impl WhisperEngine {
    /// Benchmark GPU transcription to determine optimal chunk interval.
    ///
    /// Transcribes a short silence buffer to measure the fixed overhead
    /// of the transcription pipeline. This overhead is relatively constant
    /// regardless of audio length, so it determines the minimum viable
    /// chunk size for streaming.
    ///
    /// Returns the measured overhead and recommended chunk interval.
    pub fn benchmark(&self, safety_margin: f32) -> Result<BenchmarkResult, WhisperError> {
        use crate::input::AudioBuffer;

        info!("Running GPU benchmark to determine optimal chunk interval...");

        // Generate 2 seconds of silence for benchmarking
        // This is enough to trigger full pipeline but short enough to be fast
        let test_duration_secs = 2.0f32;
        let sample_rate = 16000;
        let num_samples = (test_duration_secs * sample_rate as f32) as usize;

        // Create silence buffer (zeros)
        let samples: Vec<f32> = vec![0.0; num_samples];
        let audio = AudioBuffer {
            samples,
            sample_rate,
        };

        // Warm-up run (first run may have additional JIT overhead)
        let _ = self.transcribe(&audio);

        // Benchmark run (average of 3 runs for stability)
        let mut total_ms: u64 = 0;
        const BENCHMARK_RUNS: u32 = 3;

        for i in 0..BENCHMARK_RUNS {
            let start = std::time::Instant::now();
            let _ = self.transcribe(&audio);
            let elapsed = start.elapsed().as_millis() as u64;
            total_ms += elapsed;
            debug!("Benchmark run {}: {}ms", i + 1, elapsed);
        }

        let avg_ms = total_ms / BENCHMARK_RUNS as u64;
        let overhead_secs = avg_ms as f32 / 1000.0;

        // Calculate recommended chunk interval:
        // min_chunk = overhead * (1 + safety_margin)
        // This ensures chunks complete before the next one is ready
        let recommended = overhead_secs * (1.0 + safety_margin);

        info!(
            "Benchmark complete: {:.2}s overhead, recommended chunk interval: {:.2}s (with {:.0}% margin)",
            overhead_secs,
            recommended,
            safety_margin * 100.0
        );

        Ok(BenchmarkResult {
            overhead_secs,
            recommended_chunk_interval: recommended,
            test_audio_secs: test_duration_secs,
        })
    }
}

/// Get the model directory path
#[allow(dead_code)]
pub fn models_dir() -> Result<PathBuf, WhisperError> {
    let data_dir = Config::data_dir().map_err(|e| WhisperError::LoadFailed(e.to_string()))?;
    Ok(data_dir.join("models"))
}

/// Check if a model is downloaded
#[must_use]
#[allow(dead_code)]
pub fn is_model_downloaded(model: WhisperModel) -> bool {
    if let Ok(dir) = models_dir() {
        dir.join(model.filename()).exists()
    } else {
        false
    }
}

/// List downloaded models
#[must_use]
#[allow(dead_code)]
pub fn list_downloaded_models() -> Vec<WhisperModel> {
    all_models()
        .into_iter()
        .filter(|m| is_model_downloaded(*m))
        .collect()
}

/// Get all available models
#[must_use]
pub fn all_models() -> Vec<WhisperModel> {
    vec![
        WhisperModel::Tiny,
        WhisperModel::Base,
        WhisperModel::Small,
        WhisperModel::Medium,
        WhisperModel::LargeV3,
    ]
}

/// Get model file size in bytes (approximate)
#[must_use]
pub fn model_size_bytes(model: WhisperModel) -> u64 {
    match model {
        WhisperModel::Tiny => 75_000_000,       // ~75MB
        WhisperModel::Base => 142_000_000,      // ~142MB
        WhisperModel::Small => 466_000_000,     // ~466MB
        WhisperModel::Medium => 1_500_000_000,  // ~1.5GB
        WhisperModel::LargeV3 => 3_000_000_000, // ~3GB
    }
}

/// Format bytes as human-readable size
#[must_use]
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Download a model from Hugging Face with progress callback
pub async fn download_model<F>(
    model: WhisperModel,
    mut progress_callback: F,
) -> Result<PathBuf, WhisperError>
where
    F: FnMut(u64, u64), // (downloaded, total)
{
    let dir = models_dir()?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| WhisperError::LoadFailed(format!("Cannot create models dir: {}", e)))?;

    let dest_path = dir.join(model.filename());
    let temp_path = dir.join(format!("{}.tmp", model.filename()));

    // Check if already downloaded
    if dest_path.exists() {
        return Err(WhisperError::LoadFailed(format!(
            "Model {} already exists at {}",
            model.filename(),
            dest_path.display()
        )));
    }

    let url = model.download_url();
    let client = reqwest::Client::new();

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| WhisperError::LoadFailed(format!("Download failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(WhisperError::LoadFailed(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }

    let total_size = response.content_length().unwrap_or(model_size_bytes(model));

    // Stream to temp file
    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| WhisperError::LoadFailed(format!("Cannot create temp file: {}", e)))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    use std::io::Write;

    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| WhisperError::LoadFailed(format!("Download error: {}", e)))?;
        file.write_all(&chunk)
            .map_err(|e| WhisperError::LoadFailed(format!("Write error: {}", e)))?;
        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }

    // Rename temp to final
    std::fs::rename(&temp_path, &dest_path)
        .map_err(|e| WhisperError::LoadFailed(format!("Cannot rename temp file: {}", e)))?;

    Ok(dest_path)
}

/// Remove a downloaded model
pub fn remove_model(model: WhisperModel) -> Result<(), WhisperError> {
    let dir = models_dir()?;
    let path = dir.join(model.filename());

    if !path.exists() {
        return Err(WhisperError::LoadFailed(format!(
            "Model {} not found",
            model.filename()
        )));
    }

    std::fs::remove_file(&path)
        .map_err(|e| WhisperError::LoadFailed(format!("Cannot remove model: {}", e)))?;

    Ok(())
}

/// Convert whisper language ID to ISO 639-1 language code.
///
/// Whisper uses 0-indexed language IDs matching its internal token table.
/// This covers the most common languages; unknown IDs return "unknown".
fn lang_id_to_code(id: std::ffi::c_int) -> &'static str {
    // Language IDs from whisper.cpp (see whisper.h)
    match id {
        0 => "en",   // english
        1 => "zh",   // chinese
        2 => "de",   // german
        3 => "es",   // spanish
        4 => "ru",   // russian
        5 => "ko",   // korean
        6 => "fr",   // french
        7 => "ja",   // japanese
        8 => "pt",   // portuguese
        9 => "tr",   // turkish
        10 => "pl",  // polish
        11 => "ca",  // catalan
        12 => "nl",  // dutch
        13 => "ar",  // arabic
        14 => "sv",  // swedish
        15 => "it",  // italian
        16 => "id",  // indonesian
        17 => "hi",  // hindi
        18 => "fi",  // finnish
        19 => "vi",  // vietnamese
        20 => "he",  // hebrew
        21 => "uk",  // ukrainian
        22 => "el",  // greek
        23 => "ms",  // malay
        24 => "cs",  // czech
        25 => "ro",  // romanian
        26 => "da",  // danish
        27 => "hu",  // hungarian
        28 => "ta",  // tamil
        29 => "no",  // norwegian
        30 => "th",  // thai
        31 => "ur",  // urdu
        32 => "hr",  // croatian
        33 => "bg",  // bulgarian
        34 => "lt",  // lithuanian
        35 => "la",  // latin
        36 => "mi",  // maori
        37 => "ml",  // malayalam
        38 => "cy",  // welsh
        39 => "sk",  // slovak
        40 => "te",  // telugu
        41 => "fa",  // persian
        42 => "lv",  // latvian
        43 => "bn",  // bengali
        44 => "sr",  // serbian
        45 => "az",  // azerbaijani
        46 => "sl",  // slovenian
        47 => "kn",  // kannada
        48 => "et",  // estonian
        49 => "mk",  // macedonian
        50 => "br",  // breton
        51 => "eu",  // basque
        52 => "is",  // icelandic
        53 => "hy",  // armenian
        54 => "ne",  // nepali
        55 => "mn",  // mongolian
        56 => "bs",  // bosnian
        57 => "kk",  // kazakh
        58 => "sq",  // albanian
        59 => "sw",  // swahili
        60 => "gl",  // galician
        61 => "mr",  // marathi
        62 => "pa",  // punjabi
        63 => "si",  // sinhala
        64 => "km",  // khmer
        65 => "sn",  // shona
        66 => "yo",  // yoruba
        67 => "so",  // somali
        68 => "af",  // afrikaans
        69 => "oc",  // occitan
        70 => "ka",  // georgian
        71 => "be",  // belarusian
        72 => "tg",  // tajik
        73 => "sd",  // sindhi
        74 => "gu",  // gujarati
        75 => "am",  // amharic
        76 => "yi",  // yiddish
        77 => "lo",  // lao
        78 => "uz",  // uzbek
        79 => "fo",  // faroese
        80 => "ht",  // haitian creole
        81 => "ps",  // pashto
        82 => "tk",  // turkmen
        83 => "nn",  // nynorsk
        84 => "mt",  // maltese
        85 => "sa",  // sanskrit
        86 => "lb",  // luxembourgish
        87 => "my",  // myanmar
        88 => "bo",  // tibetan
        89 => "tl",  // tagalog
        90 => "mg",  // malagasy
        91 => "as",  // assamese
        92 => "tt",  // tatar
        93 => "haw", // hawaiian
        94 => "ln",  // lingala
        95 => "ha",  // hausa
        96 => "ba",  // bashkir
        97 => "jw",  // javanese
        98 => "su",  // sundanese
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // Model FromStr Tests
    // ===================

    #[test]
    fn test_model_from_str() {
        assert_eq!("tiny".parse(), Ok(WhisperModel::Tiny));
        assert_eq!("LARGE-V3".parse(), Ok(WhisperModel::LargeV3));
        assert_eq!("invalid".parse::<WhisperModel>(), Err(()));
    }

    #[test]
    fn test_model_from_str_all_variants() {
        assert_eq!("tiny".parse(), Ok(WhisperModel::Tiny));
        assert_eq!("base".parse(), Ok(WhisperModel::Base));
        assert_eq!("small".parse(), Ok(WhisperModel::Small));
        assert_eq!("medium".parse(), Ok(WhisperModel::Medium));
        assert_eq!("large".parse(), Ok(WhisperModel::LargeV3));
        assert_eq!("large-v3".parse(), Ok(WhisperModel::LargeV3));
        assert_eq!("largev3".parse(), Ok(WhisperModel::LargeV3));
    }

    #[test]
    fn test_model_from_str_case_insensitive() {
        assert_eq!("TINY".parse(), Ok(WhisperModel::Tiny));
        assert_eq!("Medium".parse(), Ok(WhisperModel::Medium));
        assert_eq!("LARGE-V3".parse(), Ok(WhisperModel::LargeV3));
    }

    // ===================
    // Model Filename Tests
    // ===================

    #[test]
    fn test_model_filename() {
        assert_eq!(WhisperModel::Tiny.filename(), "ggml-tiny.bin");
        assert_eq!(WhisperModel::LargeV3.filename(), "ggml-large-v3.bin");
    }

    #[test]
    fn test_model_filename_all() {
        assert_eq!(WhisperModel::Tiny.filename(), "ggml-tiny.bin");
        assert_eq!(WhisperModel::Base.filename(), "ggml-base.bin");
        assert_eq!(WhisperModel::Small.filename(), "ggml-small.bin");
        assert_eq!(WhisperModel::Medium.filename(), "ggml-medium.bin");
        assert_eq!(WhisperModel::LargeV3.filename(), "ggml-large-v3.bin");
    }

    // ===================
    // Model Size Tests
    // ===================

    #[test]
    fn test_model_size_bytes() {
        assert_eq!(WhisperModel::Tiny.size_bytes(), 75_000_000);
        assert_eq!(WhisperModel::Base.size_bytes(), 142_000_000);
        assert_eq!(WhisperModel::Small.size_bytes(), 466_000_000);
        assert_eq!(WhisperModel::Medium.size_bytes(), 1_500_000_000);
        assert_eq!(WhisperModel::LargeV3.size_bytes(), 3_000_000_000);
    }

    #[test]
    fn test_model_size_bytes_function() {
        assert_eq!(model_size_bytes(WhisperModel::Tiny), 75_000_000);
        assert_eq!(model_size_bytes(WhisperModel::LargeV3), 3_000_000_000);
    }

    // ===================
    // Format Size Tests
    // ===================

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(5120), "5 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1 MB");
        assert_eq!(format_size(75_000_000), "72 MB");
        assert_eq!(format_size(500 * 1024 * 1024), "500 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_size(3_000_000_000), "2.8 GB");
    }

    // ===================
    // All Models Tests
    // ===================

    #[test]
    fn test_all_models() {
        let models = all_models();
        assert_eq!(models.len(), 5);
        assert!(models.contains(&WhisperModel::Tiny));
        assert!(models.contains(&WhisperModel::Base));
        assert!(models.contains(&WhisperModel::Small));
        assert!(models.contains(&WhisperModel::Medium));
        assert!(models.contains(&WhisperModel::LargeV3));
    }

    // ===================
    // Language ID Tests
    // ===================

    #[test]
    fn test_lang_id_common_languages() {
        assert_eq!(lang_id_to_code(0), "en");
        assert_eq!(lang_id_to_code(2), "de");
        assert_eq!(lang_id_to_code(6), "fr");
        assert_eq!(lang_id_to_code(7), "ja");
        assert_eq!(lang_id_to_code(15), "it");
    }

    #[test]
    fn test_lang_id_unknown() {
        assert_eq!(lang_id_to_code(999), "unknown");
        assert_eq!(lang_id_to_code(-1), "unknown");
    }

    #[test]
    fn test_lang_id_all_supported() {
        // Test a sampling of languages across the range
        assert_eq!(lang_id_to_code(1), "zh"); // chinese
        assert_eq!(lang_id_to_code(10), "pl"); // polish
        assert_eq!(lang_id_to_code(20), "he"); // hebrew
        assert_eq!(lang_id_to_code(30), "th"); // thai
        assert_eq!(lang_id_to_code(50), "br"); // breton
        assert_eq!(lang_id_to_code(70), "ka"); // georgian
        assert_eq!(lang_id_to_code(93), "haw"); // hawaiian
        assert_eq!(lang_id_to_code(98), "su"); // sundanese
    }

    // ===================
    // Download URL Tests
    // ===================

    #[test]
    fn test_download_url() {
        let url = WhisperModel::Tiny.download_url();
        assert!(url.starts_with("https://huggingface.co/"));
        assert!(url.contains("ggml-tiny.bin"));
    }

    // ===================
    // TranscriptionResult Tests
    // ===================

    #[test]
    fn test_transcription_result_clone() {
        let result = TranscriptionResult {
            text: "Hello world".to_string(),
            language: "en".to_string(),
            duration_ms: 100,
        };
        let cloned = result.clone();
        assert_eq!(result.text, cloned.text);
        assert_eq!(result.language, cloned.language);
        assert_eq!(result.duration_ms, cloned.duration_ms);
    }

    // ===================
    // BenchmarkResult Tests
    // ===================

    #[test]
    fn test_benchmark_result_clone() {
        let result = BenchmarkResult {
            overhead_secs: 0.5,
            recommended_chunk_interval: 0.6,
            test_audio_secs: 2.0,
        };
        let cloned = result.clone();
        assert!((result.overhead_secs - cloned.overhead_secs).abs() < 0.001);
        assert!(
            (result.recommended_chunk_interval - cloned.recommended_chunk_interval).abs() < 0.001
        );
    }

    // ===================
    // Error Tests
    // ===================

    #[test]
    fn test_whisper_error_display() {
        let err = WhisperError::ModelNotFound(PathBuf::from("/path"), "tiny".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Model not found"));
        assert!(msg.contains("/path"));
        assert!(msg.contains("tiny"));

        let err = WhisperError::LoadFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = WhisperError::TranscriptionFailed("failed".to_string());
        assert!(err.to_string().contains("failed"));
    }
}
