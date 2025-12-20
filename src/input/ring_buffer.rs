//! Lock-free ring buffer for continuous audio capture.
//!
//! This module provides an always-on audio buffer that allows instant
//! audio extraction without stream startup delay. The buffer continuously
//! records audio in a circular fashion, and recordings are extracted
//! by marking a position and then extracting samples from mark to current.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

/// Target sample rate for Whisper (16kHz)
#[allow(dead_code)]
pub const SAMPLE_RATE: u32 = 16000;

/// Lock-free single-producer single-consumer ring buffer for audio samples.
///
/// Designed for:
/// - Producer: cpal audio callback (writes samples)
/// - Consumer: daemon (marks positions, extracts samples)
///
/// The buffer uses a power-of-2 size for fast modulo operations.
pub struct AudioRingBuffer {
    /// Underlying buffer (power of 2 size for fast modulo)
    buffer: UnsafeCell<Box<[f32]>>,
    /// Capacity (power of 2)
    capacity: usize,
    /// Mask for fast modulo (capacity - 1)
    mask: usize,
    /// Write position (only modified by producer)
    write_pos: AtomicUsize,
    /// Sample rate for time calculations
    sample_rate: u32,
    /// Sequence counter for ordering results
    sequence_counter: AtomicU64,
}

// SAFETY: AudioRingBuffer is safe to share across threads under specific conditions:
//
// 1. **Single-Producer Single-Consumer (SPSC) Design**:
//    - Producer (cpal audio callback thread): Only calls `push_samples()`
//    - Consumer (daemon main thread): Only calls `mark()`, `extract_since()`, `extract_range()`
//
// 2. **Memory Ordering Guarantees**:
//    - `write_pos` is an AtomicUsize with proper ordering:
//      - Producer uses `Release` ordering when updating write position
//      - Consumer uses `Acquire` ordering when reading write position
//    - This establishes happens-before relationship: consumer sees all writes
//      that occurred before the write_pos update
//
// 3. **No Data Races**:
//    - Producer only writes to buffer slots BEFORE updating write_pos
//    - Consumer only reads slots that are BEHIND the observed write_pos
//    - Even during wraparound, the consumer never reads a slot currently being written
//
// 4. **UnsafeCell Usage**:
//    - The `buffer` field uses UnsafeCell to allow mutation from the producer
//    - Access is coordinated through the atomic write_pos barrier
//
// 5. **Invariants**:
//    - Producer must be single-threaded (cpal callback runs on one thread)
//    - Consumer must not access slots ahead of write_pos
//    - Wraparound detection prevents reading stale data
unsafe impl Send for AudioRingBuffer {}
unsafe impl Sync for AudioRingBuffer {}

/// A marked position in the ring buffer.
///
/// Created when the user presses the hotkey, used to extract
/// audio when the hotkey is released.
#[derive(Debug, Clone)]
pub struct AudioMark {
    /// Position in the buffer when mark was created
    #[allow(dead_code)]
    position: usize,
    /// Timestamp for debugging/metrics
    #[allow(dead_code)]
    pub timestamp: Instant,
    /// Sequence ID for ordering transcription results
    pub sequence_id: u64,
}

impl PartialEq for AudioMark {
    fn eq(&self, other: &Self) -> bool {
        // Compare by sequence_id only (unique identifier)
        self.sequence_id == other.sequence_id
    }
}

impl Eq for AudioMark {}

impl AudioRingBuffer {
    /// Create a new ring buffer with the specified duration.
    ///
    /// The actual capacity will be rounded up to the nearest power of 2.
    ///
    /// # Arguments
    /// * `duration_secs` - Duration of audio to buffer (e.g., 30.0 for 30 seconds)
    /// * `sample_rate` - Sample rate in Hz (typically 16000 for Whisper)
    pub fn new(duration_secs: f32, sample_rate: u32) -> Self {
        let samples_needed = (duration_secs * sample_rate as f32) as usize;

        // Round up to next power of 2 for fast modulo
        let capacity = samples_needed.next_power_of_two();
        let mask = capacity - 1;

        tracing::info!(
            "Ring buffer: {:.1}s requested, {} samples capacity ({:.1}s actual, {:.2} MB)",
            duration_secs,
            capacity,
            capacity as f32 / sample_rate as f32,
            (capacity * std::mem::size_of::<f32>()) as f32 / 1_000_000.0
        );

        let buffer = vec![0.0f32; capacity].into_boxed_slice();

        Self {
            buffer: UnsafeCell::new(buffer),
            capacity,
            mask,
            write_pos: AtomicUsize::new(0),
            sample_rate,
            sequence_counter: AtomicU64::new(0),
        }
    }

    /// Push samples from audio callback (producer side, lock-free).
    ///
    /// This method is designed to be called from the cpal audio callback
    /// with minimal latency. It uses atomic operations for the write position
    /// and direct memory writes for samples.
    ///
    /// # Safety
    /// This method must only be called from a single producer thread.
    pub fn push_samples(&self, samples: &[f32]) {
        // Use Acquire to ensure proper memory ordering with Release store below.
        // This ensures samples written before the store are visible to consumers.
        let write = self.write_pos.load(Ordering::Acquire);

        // SAFETY: Single producer, we own the write position
        let buffer = unsafe { &mut *self.buffer.get() };

        for (i, &sample) in samples.iter().enumerate() {
            let idx = (write + i) & self.mask;
            buffer[idx] = sample;
        }

        // Update write position atomically
        self.write_pos
            .store(write.wrapping_add(samples.len()), Ordering::Release);
    }

    /// Mark the current position for later extraction.
    ///
    /// Call this when the hotkey is pressed. The returned mark can be
    /// used with `extract_since` when the hotkey is released.
    pub fn mark(&self) -> AudioMark {
        let sequence_id = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        let position = self.write_pos.load(Ordering::Acquire);

        AudioMark {
            position,
            timestamp: Instant::now(),
            sequence_id,
        }
    }

    /// Extract samples from the mark position to the current write position.
    ///
    /// Call this when the hotkey is released. Returns all samples recorded
    /// since the mark was created.
    ///
    /// # Arguments
    /// * `mark` - The mark created when recording started
    ///
    /// # Returns
    /// Vector of samples, or empty if the mark is too old (buffer wrapped)
    #[allow(dead_code)]
    pub fn extract_since(&self, mark: &AudioMark) -> Vec<f32> {
        let current_write = self.write_pos.load(Ordering::Acquire);

        // Calculate how many samples are available
        let samples_written = current_write.wrapping_sub(mark.position);

        // If more samples than capacity, buffer has wrapped and old data is lost
        let samples_available = samples_written.min(self.capacity);

        if samples_written > self.capacity {
            tracing::warn!(
                "Ring buffer wrapped during recording! Lost {} samples ({:.2}s)",
                samples_written - self.capacity,
                (samples_written - self.capacity) as f32 / self.sample_rate as f32
            );
        }

        // Calculate actual start position (may be different if wrapped)
        let start_pos = if samples_written > self.capacity {
            current_write.wrapping_sub(self.capacity)
        } else {
            mark.position
        };

        // Extract samples
        let buffer = unsafe { &*self.buffer.get() };
        let mut result = Vec::with_capacity(samples_available);

        for i in 0..samples_available {
            let idx = (start_pos + i) & self.mask;
            result.push(buffer[idx]);
        }

        tracing::debug!(
            "Extracted {} samples ({:.2}s) from ring buffer",
            result.len(),
            result.len() as f32 / self.sample_rate as f32
        );

        result
    }

    /// Get the current write position.
    ///
    /// Use this for streaming chunk extraction - save the position after
    /// extracting each chunk to know where to start the next one.
    pub fn current_position(&self) -> usize {
        self.write_pos.load(Ordering::Acquire)
    }

    /// Extract samples from one position to another.
    ///
    /// Used for streaming chunk extraction during recording.
    ///
    /// # Arguments
    /// * `from_pos` - Start position (from previous extraction)
    /// * `to_pos` - End position (usually current_position())
    ///
    /// # Returns
    /// Vector of samples, or empty if range is invalid
    pub fn extract_range(&self, from_pos: usize, to_pos: usize) -> Vec<f32> {
        let samples_requested = to_pos.wrapping_sub(from_pos);

        // Limit to capacity (in case buffer wrapped)
        let samples_available = samples_requested.min(self.capacity);

        if samples_requested > self.capacity {
            tracing::warn!(
                "Chunk extraction: buffer wrapped, requested {} samples but only {} available",
                samples_requested,
                samples_available
            );
        }

        // Calculate actual start position (may differ if wrapped)
        let start_pos = if samples_requested > self.capacity {
            to_pos.wrapping_sub(self.capacity)
        } else {
            from_pos
        };

        // Extract samples
        let buffer = unsafe { &*self.buffer.get() };
        let mut result = Vec::with_capacity(samples_available);

        for i in 0..samples_available {
            let idx = (start_pos + i) & self.mask;
            result.push(buffer[idx]);
        }

        tracing::debug!(
            "Extracted {} samples ({:.2}s) from position {} to {}",
            result.len(),
            result.len() as f32 / self.sample_rate as f32,
            from_pos,
            to_pos
        );

        result
    }

    /// Get the current write position (for debugging/metrics).
    #[allow(dead_code)]
    pub fn write_position(&self) -> usize {
        self.write_pos.load(Ordering::Relaxed)
    }

    /// Get the buffer capacity in samples.
    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the buffer duration in seconds.
    #[allow(dead_code)]
    pub fn duration_secs(&self) -> f32 {
        self.capacity as f32 / self.sample_rate as f32
    }

    /// Get the sample rate.
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_power_of_two() {
        let buffer = AudioRingBuffer::new(1.0, 16000);
        // 16000 samples -> next power of 2 is 16384
        assert_eq!(buffer.capacity, 16384);
        assert_eq!(buffer.mask, 16383);
    }

    #[test]
    fn test_push_and_extract() {
        let buffer = AudioRingBuffer::new(1.0, 16000);

        // Mark position
        let mark = buffer.mark();

        // Push some samples
        let samples: Vec<f32> = (0..1000).map(|i| i as f32 / 1000.0).collect();
        buffer.push_samples(&samples);

        // Extract
        let extracted = buffer.extract_since(&mark);

        assert_eq!(extracted.len(), 1000);
        assert!((extracted[0] - 0.0).abs() < 0.001);
        assert!((extracted[999] - 0.999).abs() < 0.001);
    }

    #[test]
    fn test_wraparound() {
        // Small buffer for testing wraparound
        let buffer = AudioRingBuffer::new(0.1, 16000); // ~1600 samples -> 2048 capacity

        // Fill buffer multiple times
        let samples: Vec<f32> = vec![0.5; 1000];
        for _ in 0..5 {
            buffer.push_samples(&samples);
        }

        // Mark and push more
        let mark = buffer.mark();
        let new_samples: Vec<f32> = (0..500).map(|i| i as f32 / 500.0).collect();
        buffer.push_samples(&new_samples);

        // Extract
        let extracted = buffer.extract_since(&mark);

        assert_eq!(extracted.len(), 500);
        assert!((extracted[0] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_sequence_ids() {
        let buffer = AudioRingBuffer::new(1.0, 16000);

        let mark1 = buffer.mark();
        let mark2 = buffer.mark();
        let mark3 = buffer.mark();

        assert_eq!(mark1.sequence_id, 0);
        assert_eq!(mark2.sequence_id, 1);
        assert_eq!(mark3.sequence_id, 2);
    }

    #[test]
    fn test_empty_extract() {
        let buffer = AudioRingBuffer::new(1.0, 16000);

        let mark = buffer.mark();
        let extracted = buffer.extract_since(&mark);

        assert!(extracted.is_empty());
    }

    #[test]
    fn test_buffer_overflow_warning() {
        // Very small buffer
        let buffer = AudioRingBuffer::new(0.01, 16000); // ~160 samples -> 256 capacity

        let mark = buffer.mark();

        // Push more than capacity
        let samples: Vec<f32> = vec![0.5; 1000];
        buffer.push_samples(&samples);

        // Extract - should get only capacity samples
        let extracted = buffer.extract_since(&mark);

        assert_eq!(extracted.len(), buffer.capacity());
    }
}
