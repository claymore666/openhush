//! Transcription queue for async processing.
//!
//! This module decouples audio capture from transcription, allowing:
//! - Multiple overlapping recordings
//! - Async transcription without blocking the main loop
//! - Ordered output regardless of completion order

pub mod worker;

use crate::input::AudioBuffer;
use std::collections::{BTreeMap, HashSet};

/// A job to be processed by the transcription worker.
#[derive(Debug)]
pub struct TranscriptionJob {
    /// Audio buffer to transcribe
    pub buffer: AudioBuffer,
    /// Sequence ID for ordering results (recording ID)
    pub sequence_id: u64,
    /// Chunk ID within the recording (0, 1, 2, ...)
    pub chunk_id: u32,
    /// True if this is the final chunk of the recording
    pub is_final: bool,
}

/// Result from a completed transcription.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Sequence ID for ordering (recording ID)
    pub sequence_id: u64,
    /// Chunk ID within the recording
    pub chunk_id: u32,
    /// True if this is the final chunk
    #[allow(dead_code)]
    pub is_final: bool,
}

/// Composite key for tracking chunks: (sequence_id, chunk_id)
type ChunkKey = (u64, u32);

/// Tracks pending and completed transcriptions to ensure ordered output.
///
/// Supports streaming mode where chunks are output immediately, or
/// ordered mode where results are buffered until complete.
///
/// # Streaming Mode
/// Each chunk is output immediately as it completes. This gives fastest
/// feedback for live dictation.
///
/// # Ordered Mode (legacy)
/// Results are buffered and output in sequence order.
#[derive(Debug, Default)]
pub struct TranscriptionTracker {
    /// Chunk keys that are currently being processed
    pending: HashSet<ChunkKey>,
    /// Results waiting to be output (keyed by (sequence_id, chunk_id))
    completed: BTreeMap<ChunkKey, TranscriptionResult>,
    /// Next sequence ID to output (for ordered mode)
    next_output_id: u64,
    /// Streaming mode: output chunks immediately
    streaming: bool,
    /// Last output text ending (for deduplication)
    last_text_suffix: String,
}

impl TranscriptionTracker {
    /// Create a new transcription tracker.
    pub fn new() -> Self {
        Self {
            streaming: true, // Default to streaming mode
            ..Self::default()
        }
    }

    /// Create a tracker in ordered (non-streaming) mode.
    #[allow(dead_code)]
    pub fn new_ordered() -> Self {
        Self {
            streaming: false,
            ..Self::default()
        }
    }

    /// Add a pending transcription job.
    ///
    /// Returns true if the job was accepted, false if it was rejected due to backpressure.
    pub fn add_pending(&mut self, sequence_id: u64, chunk_id: u32) -> bool {
        self.add_pending_with_config(sequence_id, chunk_id, 10, 8, "warn")
    }

    /// Add a pending transcription job with explicit backpressure configuration.
    ///
    /// # Arguments
    /// * `sequence_id` - Recording sequence ID
    /// * `chunk_id` - Chunk ID within the recording
    /// * `max_pending` - Maximum pending jobs (0 = unlimited)
    /// * `high_water_mark` - Warn when reaching this depth
    /// * `strategy` - Backpressure strategy: "warn", "drop_oldest", "drop_newest"
    ///
    /// # Returns
    /// * `true` if job was accepted
    /// * `false` if job was rejected (drop_newest strategy)
    pub fn add_pending_with_config(
        &mut self,
        sequence_id: u64,
        chunk_id: u32,
        max_pending: u32,
        high_water_mark: u32,
        strategy: &str,
    ) -> bool {
        let pending_count = self.pending.len();
        let max_pending = max_pending as usize;
        let high_water_mark = high_water_mark as usize;

        // Check if at capacity (0 means unlimited)
        if max_pending > 0 && pending_count >= max_pending {
            match strategy {
                "drop_oldest" => {
                    // Drop the oldest pending job to make room
                    if let Some(&oldest_key) = self.pending.iter().min() {
                        self.pending.remove(&oldest_key);
                        tracing::warn!(
                            "Backpressure: dropped oldest job (seq {}.{}) to accept (seq {}.{})",
                            oldest_key.0, oldest_key.1,
                            sequence_id, chunk_id
                        );
                    }
                }
                "drop_newest" => {
                    tracing::warn!(
                        "Backpressure: rejecting job (seq {}.{}) - queue full ({}/{})",
                        sequence_id, chunk_id, pending_count, max_pending
                    );
                    return false;
                }
                _ => {
                    // Default "warn" strategy - just warn but accept
                    tracing::warn!(
                        "Queue at capacity ({}/{}) but accepting job anyway",
                        pending_count, max_pending
                    );
                }
            }
        } else if high_water_mark > 0 && pending_count >= high_water_mark {
            // Approaching capacity - log warning
            tracing::warn!(
                "Queue depth {} approaching limit {} - transcription falling behind",
                pending_count, max_pending
            );
        }

        self.pending.insert((sequence_id, chunk_id));
        tracing::debug!(
            "Added pending transcription (seq {}.{}), queue depth: {}",
            sequence_id,
            chunk_id,
            self.pending.len()
        );

        true
    }

    /// Get current queue statistics
    #[allow(dead_code)]
    pub fn stats(&self) -> QueueStats {
        QueueStats {
            pending_count: self.pending.len(),
            waiting_count: self.completed.len(),
        }
    }

    /// Add a completed transcription result.
    pub fn add_result(&mut self, result: TranscriptionResult) {
        let key = (result.sequence_id, result.chunk_id);
        self.pending.remove(&key);
        self.completed.insert(key, result);
        tracing::debug!(
            "Added result (seq {}.{}), {} pending, {} waiting",
            key.0,
            key.1,
            self.pending.len(),
            self.completed.len()
        );
    }

    /// Take results that are ready for output.
    ///
    /// In streaming mode: returns all completed results immediately.
    /// In ordered mode: returns results in sequence order only.
    pub fn take_ready(&mut self) -> Vec<TranscriptionResult> {
        if self.streaming {
            self.take_ready_streaming()
        } else {
            self.take_ready_ordered()
        }
    }

    /// Streaming mode: take all completed results, apply deduplication.
    fn take_ready_streaming(&mut self) -> Vec<TranscriptionResult> {
        let mut ready: Vec<_> = std::mem::take(&mut self.completed).into_values().collect();

        // Sort by (sequence_id, chunk_id) for consistent ordering
        ready.sort_by_key(|r| (r.sequence_id, r.chunk_id));

        // Apply deduplication to each result
        for result in &mut ready {
            if !self.last_text_suffix.is_empty() && !result.text.is_empty() {
                result.text = self.deduplicate_text(&result.text);
            }
            // Save suffix for next deduplication (last ~50 chars)
            if result.text.len() > 10 {
                let suffix_start = result.text.len().saturating_sub(50);
                self.last_text_suffix = result.text[suffix_start..].to_string();
            }
        }

        ready
    }

    /// Ordered mode: take results in sequence order only.
    fn take_ready_ordered(&mut self) -> Vec<TranscriptionResult> {
        let mut ready = Vec::new();

        // For ordered mode, we need to wait for complete recordings
        // This is simplified - just output by sequence_id order
        while let Some(result) = self.completed.remove(&(self.next_output_id, 0)) {
            ready.push(result);
            self.next_output_id += 1;
        }

        ready
    }

    /// Remove duplicate words at the beginning of text that match the end of previous output.
    fn deduplicate_text(&self, text: &str) -> String {
        // Find overlap between last_text_suffix and start of new text
        let suffix = &self.last_text_suffix;
        let text_words: Vec<&str> = text.split_whitespace().collect();

        if text_words.is_empty() {
            return text.to_string();
        }

        // Try to find where the overlap ends
        // Look for the longest prefix of text that appears in suffix
        let mut skip_words = 0;
        for i in 1..=text_words.len().min(10) {
            let prefix: String = text_words[..i].join(" ");
            if suffix.contains(&prefix) {
                skip_words = i;
            }
        }

        if skip_words > 0 {
            tracing::debug!("Deduplicating: skipping {} words", skip_words);
            text_words[skip_words..].join(" ")
        } else {
            text.to_string()
        }
    }

    /// Reset the deduplication state (call when starting a new recording).
    pub fn reset_dedup(&mut self) {
        self.last_text_suffix.clear();
    }

    /// Check if there are any pending or buffered transcriptions.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.completed.is_empty()
    }

    /// Get the number of pending transcriptions.
    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get the number of completed but not yet output transcriptions.
    #[allow(dead_code)]
    pub fn waiting_count(&self) -> usize {
        self.completed.len()
    }
}

/// Queue statistics for monitoring
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// Number of jobs currently being transcribed
    pub pending_count: usize,
    /// Number of completed results waiting for output
    pub waiting_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(seq: u64, chunk: u32, text: &str, is_final: bool) -> TranscriptionResult {
        TranscriptionResult {
            text: text.to_string(),
            sequence_id: seq,
            chunk_id: chunk,
            is_final,
        }
    }

    #[test]
    fn test_streaming_mode_outputs_immediately() {
        let mut tracker = TranscriptionTracker::new(); // streaming by default

        tracker.add_pending(0, 0);
        tracker.add_pending(0, 1);

        // Add chunk 1 first (out of order)
        tracker.add_result(result(0, 1, "world", true));

        // In streaming mode, it outputs immediately
        let ready = tracker.take_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].text, "world");

        // Add chunk 0
        tracker.add_result(result(0, 0, "hello", false));
        let ready = tracker.take_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].text, "hello");
    }

    #[test]
    fn test_ordered_mode_waits() {
        let mut tracker = TranscriptionTracker::new_ordered();

        tracker.add_pending(0, 0);
        tracker.add_pending(1, 0);

        // Add result for seq 1 first
        tracker.add_result(result(1, 0, "second", true));

        // Should wait for seq 0
        let ready = tracker.take_ready();
        assert!(ready.is_empty());

        // Add result for seq 0
        tracker.add_result(result(0, 0, "first", true));

        // Now both seq 0 and seq 1 are ready (consecutive)
        let ready = tracker.take_ready();
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].text, "first");
        assert_eq!(ready[1].text, "second");
    }

    #[test]
    fn test_deduplication() {
        let mut tracker = TranscriptionTracker::new();

        tracker.add_pending(0, 0);
        tracker.add_result(result(0, 0, "hello world this is a test", false));
        let ready = tracker.take_ready();
        assert_eq!(ready[0].text, "hello world this is a test");

        // Next chunk has overlap
        tracker.add_pending(0, 1);
        tracker.add_result(result(0, 1, "is a test and more words", true));
        let ready = tracker.take_ready();
        // Should deduplicate "is a test"
        assert_eq!(ready[0].text, "and more words");
    }

    #[test]
    fn test_empty_tracker() {
        let tracker = TranscriptionTracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(tracker.waiting_count(), 0);
    }

    #[test]
    fn test_pending_count() {
        let mut tracker = TranscriptionTracker::new();

        tracker.add_pending(0, 0);
        tracker.add_pending(0, 1);
        assert_eq!(tracker.pending_count(), 2);

        tracker.add_result(result(0, 0, "test", false));
        assert_eq!(tracker.pending_count(), 1);
        assert_eq!(tracker.waiting_count(), 1);
    }

    #[test]
    fn test_backpressure_drop_newest() {
        let mut tracker = TranscriptionTracker::new();

        // Fill up to max_pending (3)
        assert!(tracker.add_pending_with_config(0, 0, 3, 2, "drop_newest"));
        assert!(tracker.add_pending_with_config(0, 1, 3, 2, "drop_newest"));
        assert!(tracker.add_pending_with_config(0, 2, 3, 2, "drop_newest"));

        // At capacity - next should be rejected
        assert!(!tracker.add_pending_with_config(0, 3, 3, 2, "drop_newest"));
        assert_eq!(tracker.pending_count(), 3);
    }

    #[test]
    fn test_backpressure_drop_oldest() {
        let mut tracker = TranscriptionTracker::new();

        // Fill up to max_pending (3)
        assert!(tracker.add_pending_with_config(0, 0, 3, 2, "drop_oldest"));
        assert!(tracker.add_pending_with_config(0, 1, 3, 2, "drop_oldest"));
        assert!(tracker.add_pending_with_config(0, 2, 3, 2, "drop_oldest"));

        // At capacity - oldest should be dropped, new one accepted
        assert!(tracker.add_pending_with_config(0, 3, 3, 2, "drop_oldest"));
        assert_eq!(tracker.pending_count(), 3);

        // Oldest (0,0) should have been dropped
        assert!(!tracker.pending.contains(&(0, 0)));
        assert!(tracker.pending.contains(&(0, 3)));
    }

    #[test]
    fn test_backpressure_warn_accepts() {
        let mut tracker = TranscriptionTracker::new();

        // Fill up to max_pending (3)
        assert!(tracker.add_pending_with_config(0, 0, 3, 2, "warn"));
        assert!(tracker.add_pending_with_config(0, 1, 3, 2, "warn"));
        assert!(tracker.add_pending_with_config(0, 2, 3, 2, "warn"));

        // At capacity - warn strategy still accepts
        assert!(tracker.add_pending_with_config(0, 3, 3, 2, "warn"));
        assert_eq!(tracker.pending_count(), 4);
    }

    #[test]
    fn test_queue_stats() {
        let mut tracker = TranscriptionTracker::new();

        tracker.add_pending(0, 0);
        tracker.add_pending(0, 1);
        tracker.add_result(result(0, 0, "test", false));

        let stats = tracker.stats();
        assert_eq!(stats.pending_count, 1);
        assert_eq!(stats.waiting_count, 1);
    }
}
