//! Fuzz target for audio processing operations.
//!
//! Tests AudioBuffer methods that perform signal processing,
//! ensuring no panics on edge cases like empty buffers, NaN values, etc.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use openhush::AudioBuffer;

/// Audio processing operations to fuzz.
#[derive(Arbitrary, Debug)]
enum AudioOp {
    /// RMS normalization to target dB level.
    NormalizeRms { target_db: f32 },
    /// Apply gain in dB.
    ApplyGain { gain_db: f32 },
    /// Dynamic range compression.
    Compress {
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    },
    /// Hard/soft limiting.
    Limit { ceiling_db: f32, release_ms: f32 },
    /// Calculate RMS level.
    RmsDb,
    /// Calculate duration.
    Duration,
}

fuzz_target!(|data: (Vec<f32>, Vec<AudioOp>)| {
    let (samples, ops) = data;

    // Limit size to prevent OOM
    if samples.len() > 16000 * 60 {
        return;
    }

    let mut buffer = AudioBuffer {
        samples,
        sample_rate: 16000,
    };

    for op in ops {
        match op {
            AudioOp::NormalizeRms { target_db } => {
                buffer.normalize_rms(target_db);
            }
            AudioOp::ApplyGain { gain_db } => {
                buffer.apply_gain(gain_db);
            }
            AudioOp::Compress {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_gain_db,
            } => {
                buffer.compress(threshold_db, ratio, attack_ms, release_ms, makeup_gain_db);
            }
            AudioOp::Limit {
                ceiling_db,
                release_ms,
            } => {
                buffer.limit(ceiling_db, release_ms);
            }
            AudioOp::RmsDb => {
                let _ = buffer.rms_db();
            }
            AudioOp::Duration => {
                let _ = buffer.duration_secs();
            }
        }
    }
});
