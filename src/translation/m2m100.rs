//! M2M-100 neural translation engine.
//!
//! M2M-100 is a multilingual encoder-decoder model that can translate
//! directly between 100 languages without relying on English as an
//! intermediate language.
//!
//! Model sizes:
//! - M2M-100 418M: ~1.5GB, good quality, fast
//! - M2M-100 1.2B: ~4.5GB, excellent quality, slower
//!
//! Uses ONNX Runtime for inference with the Hugging Face tokenizers library.

// M2M-100 engine is implemented but not yet integrated into the daemon
#![allow(dead_code)]

use super::{is_m2m100_language, TranslationEngine, TranslationError};
use crate::config::Config;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokenizers::Tokenizer;
use tracing::{debug, info, warn};

/// Maximum sequence length for translation.
const MAX_LENGTH: usize = 256;

/// M2M-100 specific errors.
#[derive(Error, Debug)]
pub enum M2M100Error {
    #[error("Model not found at {0}")]
    ModelNotFound(PathBuf),

    #[error("Model not downloaded. Run: openhush model download m2m100-418m")]
    ModelNotDownloaded,

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("ONNX error: {0}")]
    Onnx(String),
}

impl From<ort::Error> for M2M100Error {
    fn from(err: ort::Error) -> Self {
        M2M100Error::Onnx(err.to_string())
    }
}

/// M2M-100 model variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum M2M100Model {
    /// 418M parameter model (~1.5GB)
    #[default]
    Small,
    /// 1.2B parameter model (~4.5GB)
    Large,
}

impl M2M100Model {
    /// Get the model name for downloads.
    pub fn name(&self) -> &str {
        match self {
            Self::Small => "m2m100-418m",
            Self::Large => "m2m100-1.2b",
        }
    }

    /// Get the Hugging Face model ID.
    pub fn hf_model_id(&self) -> &str {
        match self {
            Self::Small => "facebook/m2m100_418M",
            Self::Large => "facebook/m2m100_1.2B",
        }
    }

    /// Get the ONNX model repository URL (pre-converted).
    pub fn onnx_repo(&self) -> &str {
        match self {
            Self::Small => "optimum/m2m100_418M",
            Self::Large => {
                // 1.2B needs custom conversion
                "optimum/m2m100_1.2B"
            }
        }
    }

    /// Estimated VRAM usage in MB.
    pub fn vram_mb(&self) -> u32 {
        match self {
            Self::Small => 1500,
            Self::Large => 4500,
        }
    }
}

impl std::str::FromStr for M2M100Model {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "418m" | "small" | "m2m100-418m" => Ok(Self::Small),
            "1.2b" | "large" | "m2m100-1.2b" => Ok(Self::Large),
            _ => Err(format!("Unknown M2M-100 model: {}", s)),
        }
    }
}

/// Files required for M2M-100 model.
const MODEL_FILES: &[&str] = &["encoder_model.onnx", "decoder_model.onnx", "tokenizer.json"];

/// Get the models directory for M2M-100.
pub fn models_dir() -> Result<PathBuf, M2M100Error> {
    Config::data_dir()
        .map(|d| d.join("models").join("m2m100"))
        .map_err(|e| M2M100Error::Inference(format!("Cannot get data dir: {}", e)))
}

/// Get the model directory for a specific variant.
pub fn model_dir(model: M2M100Model) -> Result<PathBuf, M2M100Error> {
    models_dir().map(|d| d.join(model.name()))
}

/// Check if a model is downloaded.
pub fn is_model_downloaded(model: M2M100Model) -> bool {
    if let Ok(dir) = model_dir(model) {
        MODEL_FILES.iter().all(|f| dir.join(f).exists())
    } else {
        false
    }
}

/// Download M2M-100 model files from HuggingFace.
///
/// Downloads encoder, decoder, and tokenizer files with progress callback.
pub async fn download_model<F>(
    model: M2M100Model,
    mut progress_callback: F,
) -> Result<PathBuf, M2M100Error>
where
    F: FnMut(&str, u64, u64), // (filename, downloaded, total)
{
    let dir = model_dir(model)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| M2M100Error::Inference(format!("Cannot create models dir: {}", e)))?;

    // Check if already downloaded
    if is_model_downloaded(model) {
        return Err(M2M100Error::Inference(format!(
            "Model {} already exists at {}",
            model.name(),
            dir.display()
        )));
    }

    // HuggingFace URL pattern for ONNX models
    // Note: M2M-100 ONNX models need to be exported with optimum-cli
    // This downloads from the optimum namespace which has pre-converted models
    let base_url = format!(
        "https://huggingface.co/{}/resolve/main/onnx",
        model.onnx_repo()
    );

    let client = reqwest::Client::builder()
        .user_agent("openhush/0.6.0")
        .build()
        .map_err(|e| M2M100Error::Inference(format!("HTTP client error: {}", e)))?;

    for filename in MODEL_FILES {
        let url = if *filename == "tokenizer.json" {
            // Tokenizer is in the root, not onnx subdirectory
            format!(
                "https://huggingface.co/{}/resolve/main/tokenizer.json",
                model.onnx_repo()
            )
        } else {
            format!("{}/{}", base_url, filename)
        };

        let dest_path = dir.join(filename);
        let temp_path = dir.join(format!("{}.tmp", filename));

        // Skip if already exists
        if dest_path.exists() {
            info!("{} already exists, skipping", filename);
            continue;
        }

        info!("Downloading {} from {}", filename, url);

        let response =
            client.get(&url).send().await.map_err(|e| {
                M2M100Error::Inference(format!("Download {} failed: {}", filename, e))
            })?;

        if !response.status().is_success() {
            // Check if this is a 404 - model might not be available
            if response.status().as_u16() == 404 {
                // Clean up any partial downloads
                let _ = std::fs::remove_dir_all(&dir);
                return Err(M2M100Error::Inference(format!(
                    "Model {} not found on HuggingFace.\n\
                     ONNX models may need to be exported manually using:\n\n\
                     pip install optimum[exporters]\n\
                     optimum-cli export onnx --model {} {}/\n\n\
                     Then copy the files to: {}",
                    model.name(),
                    model.hf_model_id(),
                    model.name(),
                    dir.display()
                )));
            }
            return Err(M2M100Error::Inference(format!(
                "Download {} failed with status: {}",
                filename,
                response.status()
            )));
        }

        let total_size = response.content_length().unwrap_or(0);

        // Stream to temp file
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| M2M100Error::Inference(format!("Cannot create temp file: {}", e)))?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        use std::io::Write;

        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|e| M2M100Error::Inference(format!("Download error: {}", e)))?;
            file.write_all(&chunk)
                .map_err(|e| M2M100Error::Inference(format!("Write error: {}", e)))?;
            downloaded += chunk.len() as u64;
            progress_callback(filename, downloaded, total_size);
        }

        // Rename temp to final
        std::fs::rename(&temp_path, &dest_path)
            .map_err(|e| M2M100Error::Inference(format!("Cannot rename temp file: {}", e)))?;

        info!("Downloaded {}", filename);
    }

    Ok(dir)
}

/// Remove a downloaded M2M-100 model.
pub fn remove_model(model: M2M100Model) -> Result<(), M2M100Error> {
    let dir = model_dir(model)?;

    if !dir.exists() {
        return Err(M2M100Error::ModelNotFound(dir));
    }

    std::fs::remove_dir_all(&dir)
        .map_err(|e| M2M100Error::Inference(format!("Cannot remove model: {}", e)))?;

    info!("Removed {} model from {}", model.name(), dir.display());
    Ok(())
}

/// List downloaded M2M-100 models with their sizes.
pub fn list_models() -> Result<Vec<(M2M100Model, u64)>, M2M100Error> {
    let mut result = Vec::new();

    for model in [M2M100Model::Small, M2M100Model::Large] {
        if is_model_downloaded(model) {
            if let Ok(dir) = model_dir(model) {
                let size = dir_size(&dir);
                result.push((model, size));
            }
        }
    }

    Ok(result)
}

/// Calculate total size of a directory.
fn dir_size(path: &Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    size += metadata.len();
                }
            }
        }
    }
    size
}

/// Language code to token ID mapping.
/// M2M-100 uses tokens like __en__, __de__, etc. with IDs 128004-128103.
fn lang_to_token_id(lang: &str) -> Option<i64> {
    // M2M-100 language token IDs (from tokenizer_config.json)
    // Format: __XX__ where XX is ISO 639-1 code
    let lang_lower = lang.to_lowercase();
    match lang_lower.as_str() {
        "af" => Some(128_006),
        "am" => Some(128_007),
        "ar" => Some(128_008),
        "ast" => Some(128_009),
        "az" => Some(128_010),
        "ba" => Some(128_011),
        "be" => Some(128_012),
        "bg" => Some(128_013),
        "bn" => Some(128_014),
        "br" => Some(128_015),
        "bs" => Some(128_016),
        "ca" => Some(128_017),
        "ceb" => Some(128_018),
        "cs" => Some(128_019),
        "cy" => Some(128_020),
        "da" => Some(128_021),
        "de" => Some(128_022),
        "el" => Some(128_023),
        "en" => Some(128_024),
        "es" => Some(128_025),
        "et" => Some(128_026),
        "fa" => Some(128_027),
        "ff" => Some(128_028),
        "fi" => Some(128_029),
        "fr" => Some(128_030),
        "fy" => Some(128_031),
        "ga" => Some(128_032),
        "gd" => Some(128_033),
        "gl" => Some(128_034),
        "gu" => Some(128_035),
        "ha" => Some(128_036),
        "he" => Some(128_037),
        "hi" => Some(128_038),
        "hr" => Some(128_039),
        "ht" => Some(128_040),
        "hu" => Some(128_041),
        "hy" => Some(128_042),
        "id" => Some(128_043),
        "ig" => Some(128_044),
        "ilo" => Some(128_045),
        "is" => Some(128_046),
        "it" => Some(128_047),
        "ja" => Some(128_048),
        "jv" => Some(128_049),
        "ka" => Some(128_050),
        "kk" => Some(128_051),
        "km" => Some(128_052),
        "kn" => Some(128_053),
        "ko" => Some(128_054),
        "lb" => Some(128_055),
        "lg" => Some(128_056),
        "ln" => Some(128_057),
        "lo" => Some(128_058),
        "lt" => Some(128_059),
        "lv" => Some(128_060),
        "mg" => Some(128_061),
        "mk" => Some(128_062),
        "ml" => Some(128_063),
        "mn" => Some(128_064),
        "mr" => Some(128_065),
        "ms" => Some(128_066),
        "my" => Some(128_067),
        "ne" => Some(128_068),
        "nl" => Some(128_069),
        "no" => Some(128_070),
        "ns" => Some(128_071),
        "oc" => Some(128_072),
        "or" => Some(128_073),
        "pa" => Some(128_074),
        "pl" => Some(128_075),
        "ps" => Some(128_076),
        "pt" => Some(128_077),
        "ro" => Some(128_078),
        "ru" => Some(128_079),
        "sd" => Some(128_080),
        "si" => Some(128_081),
        "sk" => Some(128_082),
        "sl" => Some(128_083),
        "so" => Some(128_084),
        "sq" => Some(128_085),
        "sr" => Some(128_086),
        "ss" => Some(128_087),
        "su" => Some(128_088),
        "sv" => Some(128_089),
        "sw" => Some(128_090),
        "ta" => Some(128_091),
        "th" => Some(128_092),
        "tl" => Some(128_093),
        "tn" => Some(128_094),
        "tr" => Some(128_095),
        "uk" => Some(128_096),
        "ur" => Some(128_097),
        "uz" => Some(128_098),
        "vi" => Some(128_099),
        "wo" => Some(128_100),
        "xh" => Some(128_101),
        "yi" => Some(128_102),
        "yo" => Some(128_103),
        "zh" => Some(128_104),
        "zu" => Some(128_105),
        _ => None,
    }
}

/// M2M-100 translation engine using ONNX Runtime.
pub struct M2M100Engine {
    model_variant: M2M100Model,
    model_dir: PathBuf,
    tokenizer: Option<Tokenizer>,
    encoder: Option<Mutex<Session>>,
    decoder: Option<Mutex<Session>>,
}

impl M2M100Engine {
    /// Create a new M2M-100 engine.
    pub fn new(model: M2M100Model, model_dir: PathBuf) -> Self {
        Self {
            model_variant: model,
            model_dir,
            tokenizer: None,
            encoder: None,
            decoder: None,
        }
    }

    /// Check if the model files exist.
    pub fn is_downloaded(&self) -> bool {
        let encoder_path = self.model_dir.join("encoder_model.onnx");
        let decoder_path = self.model_dir.join("decoder_model.onnx");
        let tokenizer_path = self.model_dir.join("tokenizer.json");

        encoder_path.exists() && decoder_path.exists() && tokenizer_path.exists()
    }

    /// Get the model directory path.
    pub fn model_dir(&self) -> &PathBuf {
        &self.model_dir
    }

    /// Get the model variant.
    pub fn model(&self) -> M2M100Model {
        self.model_variant
    }

    /// Check if the model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.tokenizer.is_some() && self.encoder.is_some() && self.decoder.is_some()
    }

    /// Load the model into memory.
    pub fn load(&mut self) -> Result<(), M2M100Error> {
        if !self.is_downloaded() {
            return Err(M2M100Error::ModelNotDownloaded);
        }

        // Load tokenizer
        let tokenizer_path = self.model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| M2M100Error::Tokenizer(e.to_string()))?;
        self.tokenizer = Some(tokenizer);

        // Load encoder model
        let encoder_path = self.model_dir.join("encoder_model.onnx");
        let encoder = Session::builder()
            .map_err(|e| M2M100Error::Onnx(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| M2M100Error::Onnx(e.to_string()))?
            .with_intra_threads(4)
            .map_err(|e| M2M100Error::Onnx(e.to_string()))?
            .commit_from_file(&encoder_path)
            .map_err(|e: ort::Error| M2M100Error::Onnx(format!("encoder: {}", e)))?;
        self.encoder = Some(Mutex::new(encoder));

        // Load decoder model
        let decoder_path = self.model_dir.join("decoder_model.onnx");
        let decoder = Session::builder()
            .map_err(|e| M2M100Error::Onnx(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| M2M100Error::Onnx(e.to_string()))?
            .with_intra_threads(4)
            .map_err(|e| M2M100Error::Onnx(e.to_string()))?
            .commit_from_file(&decoder_path)
            .map_err(|e: ort::Error| M2M100Error::Onnx(format!("decoder: {}", e)))?;
        self.decoder = Some(Mutex::new(decoder));

        debug!(
            "M2M-100 {} loaded from {:?}",
            self.model_variant.name(),
            self.model_dir
        );

        Ok(())
    }

    /// Unload the model from memory.
    pub fn unload(&mut self) {
        self.tokenizer = None;
        self.encoder = None;
        self.decoder = None;
        debug!("M2M-100 model unloaded");
    }

    /// Translate text from source to target language.
    pub fn translate_sync(&self, text: &str, from: &str, to: &str) -> Result<String, M2M100Error> {
        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or(M2M100Error::Inference("Tokenizer not loaded".to_string()))?;
        let encoder_mutex = self
            .encoder
            .as_ref()
            .ok_or(M2M100Error::Inference("Encoder not loaded".to_string()))?;
        let decoder_mutex = self
            .decoder
            .as_ref()
            .ok_or(M2M100Error::Inference("Decoder not loaded".to_string()))?;

        // Lock the sessions for this translation
        let mut encoder = encoder_mutex
            .lock()
            .map_err(|_| M2M100Error::Inference("Encoder lock poisoned".to_string()))?;
        let mut decoder = decoder_mutex
            .lock()
            .map_err(|_| M2M100Error::Inference("Decoder lock poisoned".to_string()))?;

        // Get language token IDs
        let src_lang_id = lang_to_token_id(from)
            .ok_or_else(|| M2M100Error::UnsupportedLanguage(from.to_string()))?;
        let tgt_lang_id =
            lang_to_token_id(to).ok_or_else(|| M2M100Error::UnsupportedLanguage(to.to_string()))?;

        // Tokenize input
        let encoding = tokenizer
            .encode(text, false)
            .map_err(|e| M2M100Error::Tokenizer(e.to_string()))?;

        // Build input_ids: [src_lang_id, ...tokens..., eos_id]
        let eos_id: i64 = 2; // </s> token
        let mut input_ids: Vec<i64> = vec![src_lang_id];
        input_ids.extend(encoding.get_ids().iter().map(|&id| id as i64));
        input_ids.push(eos_id);

        let seq_len = input_ids.len();
        let attention_mask: Vec<i64> = vec![1; seq_len];

        // Create input tensors for encoder
        let input_ids_tensor = Tensor::from_array(([1, seq_len], input_ids.clone()))
            .map_err(|e| M2M100Error::Inference(format!("create input_ids: {}", e)))?;
        let attention_mask_tensor = Tensor::from_array(([1, seq_len], attention_mask.clone()))
            .map_err(|e| M2M100Error::Inference(format!("create attention_mask: {}", e)))?;

        // Run encoder
        let encoder_outputs = encoder
            .run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor.clone()
            ])
            .map_err(|e: ort::Error| M2M100Error::Inference(format!("encoder run: {}", e)))?;

        // Get encoder hidden states - shape: [batch, seq_len, hidden_dim]
        let (encoder_shape, encoder_hidden): (&ort::tensor::Shape, &[f32]) = encoder_outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e: ort::Error| M2M100Error::Inference(format!("extract encoder: {}", e)))?;

        let hidden_dim = if encoder_shape.len() > 2 {
            encoder_shape[2] as usize
        } else {
            return Err(M2M100Error::Inference(
                "Invalid encoder output shape".to_string(),
            ));
        };

        // Initialize decoder input with target language token
        // M2M-100 decoder starts with: [eos, tgt_lang, ...]
        let mut decoder_input_ids: Vec<i64> = vec![eos_id, tgt_lang_id];
        let mut generated_tokens: Vec<i64> = vec![];

        // Greedy decoding loop
        for _ in 0..MAX_LENGTH {
            let decoder_seq_len = decoder_input_ids.len();

            // Create decoder inputs
            let decoder_input_tensor =
                Tensor::from_array(([1, decoder_seq_len], decoder_input_ids.clone()))
                    .map_err(|e| M2M100Error::Inference(format!("create decoder input: {}", e)))?;

            let decoder_attention: Vec<i64> = vec![1; decoder_seq_len];
            let decoder_attention_tensor =
                Tensor::from_array(([1, decoder_seq_len], decoder_attention))
                    .map_err(|e| M2M100Error::Inference(format!("create decoder attn: {}", e)))?;

            // Re-create encoder hidden states tensor for this iteration
            let encoder_hidden_tensor =
                Tensor::from_array(([1, seq_len, hidden_dim], encoder_hidden.to_vec()))
                    .map_err(|e| M2M100Error::Inference(format!("create encoder hidden: {}", e)))?;

            let encoder_attention_tensor =
                Tensor::from_array(([1, seq_len], attention_mask.clone()))
                    .map_err(|e| M2M100Error::Inference(format!("create encoder attn: {}", e)))?;

            // Run decoder
            let decoder_outputs = decoder
                .run(ort::inputs![
                    "input_ids" => decoder_input_tensor,
                    "attention_mask" => decoder_attention_tensor,
                    "encoder_hidden_states" => encoder_hidden_tensor,
                    "encoder_attention_mask" => encoder_attention_tensor
                ])
                .map_err(|e: ort::Error| M2M100Error::Inference(format!("decoder run: {}", e)))?;

            // Get logits - shape: [batch, decoder_seq_len, vocab_size]
            let (logits_shape, logits_slice): (&ort::tensor::Shape, &[f32]) = decoder_outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e: ort::Error| {
                    M2M100Error::Inference(format!("extract logits: {}", e))
                })?;

            if logits_shape.len() < 3 {
                return Err(M2M100Error::Inference(
                    "Invalid decoder output shape".to_string(),
                ));
            }

            let vocab_size = logits_shape[2] as usize;
            let last_pos = decoder_seq_len - 1;

            // Find argmax at last position
            let offset = last_pos * vocab_size;
            let mut max_val = f32::NEG_INFINITY;
            let mut next_token: i64 = 0;

            for v in 0..vocab_size {
                if let Some(&val) = logits_slice.get(offset + v) {
                    if val > max_val {
                        max_val = val;
                        next_token = v as i64;
                    }
                }
            }

            // Check for EOS
            if next_token == eos_id {
                break;
            }

            generated_tokens.push(next_token);
            decoder_input_ids.push(next_token);
        }

        // Decode generated tokens
        let output = tokenizer
            .decode(
                &generated_tokens
                    .iter()
                    .map(|&x| x as u32)
                    .collect::<Vec<_>>(),
                true,
            )
            .map_err(|e| M2M100Error::Tokenizer(e.to_string()))?;

        Ok(output)
    }
}

impl TranslationEngine for M2M100Engine {
    async fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, TranslationError> {
        // Validate languages
        if !is_m2m100_language(from) && from != "auto" {
            return Err(TranslationError::M2M100(M2M100Error::UnsupportedLanguage(
                from.to_string(),
            )));
        }
        if !is_m2m100_language(to) {
            return Err(TranslationError::M2M100(M2M100Error::UnsupportedLanguage(
                to.to_string(),
            )));
        }

        if !self.is_loaded() {
            return Err(TranslationError::ModelNotLoaded);
        }

        // For "auto" source language, default to English
        // TODO: Implement language detection
        let actual_from = if from == "auto" {
            warn!("M2M-100 doesn't support auto language detection, defaulting to 'en'");
            "en"
        } else {
            from
        };

        // Run translation (blocking, should be called from blocking context)
        self.translate_sync(text, actual_from, to)
            .map_err(TranslationError::M2M100)
    }

    fn supports_pair(&self, from: &str, to: &str) -> bool {
        let from_supported = from == "auto" || is_m2m100_language(from);
        let to_supported = is_m2m100_language(to);
        from_supported && to_supported
    }

    fn name(&self) -> &str {
        "m2m100"
    }
}

/// Create a shared M2M-100 engine instance.
pub fn create_m2m100_engine(
    model: M2M100Model,
    model_dir: PathBuf,
) -> Result<Arc<M2M100Engine>, M2M100Error> {
    let mut engine = M2M100Engine::new(model, model_dir);
    engine.load()?;
    Ok(Arc::new(engine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m2m100_model_from_str() {
        assert_eq!("418m".parse::<M2M100Model>().unwrap(), M2M100Model::Small);
        assert_eq!("1.2b".parse::<M2M100Model>().unwrap(), M2M100Model::Large);
        assert!("invalid".parse::<M2M100Model>().is_err());
    }

    #[test]
    fn test_m2m100_model_name() {
        assert_eq!(M2M100Model::Small.name(), "m2m100-418m");
        assert_eq!(M2M100Model::Large.name(), "m2m100-1.2b");
    }

    #[test]
    fn test_m2m100_vram() {
        assert_eq!(M2M100Model::Small.vram_mb(), 1500);
        assert_eq!(M2M100Model::Large.vram_mb(), 4500);
    }

    #[test]
    fn test_m2m100_engine_supports_pair() {
        let engine = M2M100Engine::new(M2M100Model::Small, PathBuf::from("/tmp/m2m100"));
        assert!(engine.supports_pair("en", "de"));
        assert!(engine.supports_pair("zh", "fr"));
        assert!(engine.supports_pair("auto", "de"));
        assert!(!engine.supports_pair("xyz", "en"));
    }

    #[test]
    fn test_lang_to_token_id() {
        assert_eq!(lang_to_token_id("en"), Some(128_024));
        assert_eq!(lang_to_token_id("de"), Some(128_022));
        assert_eq!(lang_to_token_id("fr"), Some(128_030));
        assert_eq!(lang_to_token_id("zh"), Some(128_104));
        assert_eq!(lang_to_token_id("invalid"), None);
    }

    #[test]
    fn test_m2m100_error_display() {
        let err = M2M100Error::ModelNotDownloaded;
        assert!(err.to_string().contains("openhush model download"));
    }
}
