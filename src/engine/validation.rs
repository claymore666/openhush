//! Audio validation before FFI boundary.
//!
//! Validates audio data before passing it to the Whisper C++ library,
//! preventing potential crashes or undefined behavior from malformed input.

use thiserror::Error;

/// Maximum audio duration in seconds (5 minutes)
pub const MAX_AUDIO_DURATION_SECS: f32 = 300.0;

/// Minimum audio duration in seconds (100ms)
pub const MIN_AUDIO_DURATION_SECS: f32 = 0.1;

/// Expected sample rate for Whisper
pub const EXPECTED_SAMPLE_RATE: u32 = 16000;

#[derive(Error, Debug)]
pub enum AudioValidationError {
    #[error("Audio too long: {0:.1}s exceeds maximum {1:.1}s")]
    TooLong(f32, f32),

    #[error("Audio too short: {0:.3}s below minimum {1:.3}s")]
    TooShort(f32, f32),

    #[error("Audio is empty (no samples)")]
    Empty,

    #[error("Audio contains {0} NaN values")]
    ContainsNaN(usize),

    #[error("Audio contains {0} infinite values")]
    ContainsInfinite(usize),

    #[error("Unexpected sample rate: {0}Hz (expected {1}Hz)")]
    InvalidSampleRate(u32, u32),
}

/// Validate audio samples before passing to FFI.
///
/// Checks for:
/// - Empty buffer
/// - Duration limits (too short or too long)
/// - NaN values
/// - Infinite values
/// - Sample rate (if provided)
pub fn validate_audio(
    samples: &[f32],
    sample_rate: u32,
) -> Result<AudioValidationInfo, AudioValidationError> {
    // Check for empty buffer
    if samples.is_empty() {
        return Err(AudioValidationError::Empty);
    }

    // Check sample rate
    if sample_rate != EXPECTED_SAMPLE_RATE {
        return Err(AudioValidationError::InvalidSampleRate(
            sample_rate,
            EXPECTED_SAMPLE_RATE,
        ));
    }

    // Calculate duration
    let duration_secs = samples.len() as f32 / sample_rate as f32;

    // Check duration limits
    if duration_secs > MAX_AUDIO_DURATION_SECS {
        return Err(AudioValidationError::TooLong(
            duration_secs,
            MAX_AUDIO_DURATION_SECS,
        ));
    }

    if duration_secs < MIN_AUDIO_DURATION_SECS {
        return Err(AudioValidationError::TooShort(
            duration_secs,
            MIN_AUDIO_DURATION_SECS,
        ));
    }

    // Check for NaN and Infinite values
    let mut nan_count = 0;
    let mut inf_count = 0;
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    let mut sum_squares = 0.0f64;

    for &sample in samples {
        if sample.is_nan() {
            nan_count += 1;
        } else if sample.is_infinite() {
            inf_count += 1;
        } else {
            min_val = min_val.min(sample);
            max_val = max_val.max(sample);
            sum_squares += (sample as f64).powi(2);
        }
    }

    if nan_count > 0 {
        return Err(AudioValidationError::ContainsNaN(nan_count));
    }

    if inf_count > 0 {
        return Err(AudioValidationError::ContainsInfinite(inf_count));
    }

    // Calculate RMS for info
    let rms = (sum_squares / samples.len() as f64).sqrt() as f32;

    Ok(AudioValidationInfo {
        duration_secs,
        sample_count: samples.len(),
        min_value: min_val,
        max_value: max_val,
        rms,
    })
}

/// Information about validated audio.
#[derive(Debug, Clone)]
pub struct AudioValidationInfo {
    /// Duration in seconds
    pub duration_secs: f32,
    /// Number of samples
    pub sample_count: usize,
    /// Minimum sample value
    pub min_value: f32,
    /// Maximum sample value
    pub max_value: f32,
    /// RMS (root mean square) level
    pub rms: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_audio() {
        // 1 second of silence at 16kHz
        let samples = vec![0.0f32; 16000];
        let result = validate_audio(&samples, 16000);
        assert!(result.is_ok());
        let info = result.unwrap();
        assert!((info.duration_secs - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_audio() {
        let samples: Vec<f32> = vec![];
        let result = validate_audio(&samples, 16000);
        assert!(matches!(result, Err(AudioValidationError::Empty)));
    }

    #[test]
    fn test_too_short() {
        // 50ms (below 100ms minimum)
        let samples = vec![0.0f32; 800];
        let result = validate_audio(&samples, 16000);
        assert!(matches!(result, Err(AudioValidationError::TooShort(_, _))));
    }

    #[test]
    fn test_too_long() {
        // 301 seconds (above 300s maximum)
        let samples = vec![0.0f32; 16000 * 301];
        let result = validate_audio(&samples, 16000);
        assert!(matches!(result, Err(AudioValidationError::TooLong(_, _))));
    }

    #[test]
    fn test_contains_nan() {
        let mut samples = vec![0.0f32; 16000];
        samples[500] = f32::NAN;
        samples[1000] = f32::NAN;
        let result = validate_audio(&samples, 16000);
        assert!(matches!(result, Err(AudioValidationError::ContainsNaN(2))));
    }

    #[test]
    fn test_contains_infinite() {
        let mut samples = vec![0.0f32; 16000];
        samples[500] = f32::INFINITY;
        let result = validate_audio(&samples, 16000);
        assert!(matches!(
            result,
            Err(AudioValidationError::ContainsInfinite(1))
        ));
    }

    #[test]
    fn test_wrong_sample_rate() {
        let samples = vec![0.0f32; 44100]; // 1 second at 44.1kHz
        let result = validate_audio(&samples, 44100);
        assert!(matches!(
            result,
            Err(AudioValidationError::InvalidSampleRate(44100, 16000))
        ));
    }
}
