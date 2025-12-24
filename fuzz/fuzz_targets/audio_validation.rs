//! Fuzz target for audio validation.
//!
//! Tests the FFI boundary validation logic with arbitrary audio data.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use openhush::validate_audio;

/// Input structure for audio validation fuzzing.
#[derive(Arbitrary, Debug)]
struct FuzzInput {
    /// Audio samples (may contain NaN, Inf, etc.)
    samples: Vec<f32>,
    /// Maximum duration in seconds
    max_duration_secs: f32,
    /// Minimum duration in seconds
    min_duration_secs: f32,
}

fuzz_target!(|input: FuzzInput| {
    // Limit sample count to prevent OOM
    if input.samples.len() > 16000 * 300 {
        return;
    }

    // Should not panic on any input
    let _ = validate_audio(
        &input.samples,
        input.max_duration_secs,
        input.min_duration_secs,
    );
});
