//! Fuzz target for ring buffer operations.
//!
//! Tests push/extract operations with arbitrary inputs to find
//! edge cases in buffer management.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use openhush::AudioRingBuffer;

/// Operations that can be performed on the ring buffer.
#[derive(Arbitrary, Debug)]
enum Operation {
    /// Push samples into the buffer.
    Push { samples: Vec<f32> },
    /// Extract a range of samples.
    ExtractChunk { start: usize, end: usize },
    /// Mark current position.
    Mark,
    /// Get current position.
    Position,
}

fuzz_target!(|ops: Vec<Operation>| {
    // Create a buffer with a reasonable size (60 seconds at 16kHz)
    let mut buffer = AudioRingBuffer::new(16000 * 60);

    for op in ops {
        match op {
            Operation::Push { samples } => {
                // Limit push size to prevent OOM
                if samples.len() <= 16000 * 10 {
                    buffer.push_samples(&samples);
                }
            }
            Operation::ExtractChunk { start, end } => {
                // Should not panic on any start/end combination
                let _ = buffer.extract_chunk(start, end);
            }
            Operation::Mark => {
                let _ = buffer.mark();
            }
            Operation::Position => {
                let _ = buffer.current_position();
            }
        }
    }
});
