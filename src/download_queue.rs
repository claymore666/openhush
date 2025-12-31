//! Download queue with priority ordering.
//!
//! Manages model downloads with priority-based ordering to avoid bandwidth
//! contention. Higher priority downloads are processed first.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{debug, info};

/// Download priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum DownloadPriority {
    /// No download in progress
    None = 0,
    /// Low priority (M2M-100 translation model)
    Low = 1,
    /// Medium priority (Wake word models)
    Medium = 2,
    /// High priority (Whisper transcription model)
    High = 3,
}

impl From<u8> for DownloadPriority {
    fn from(val: u8) -> Self {
        match val {
            3 => DownloadPriority::High,
            2 => DownloadPriority::Medium,
            1 => DownloadPriority::Low,
            _ => DownloadPriority::None,
        }
    }
}

/// Global download queue for managing model downloads.
///
/// Uses atomic operations to track the current download priority.
/// Lower priority downloads wait for higher priority ones to complete.
#[derive(Debug)]
pub struct DownloadQueue {
    /// Current download priority (0 = none, 1 = low, 2 = medium, 3 = high)
    current_priority: AtomicU8,
    /// Notify waiters when a download completes
    notify: Notify,
}

impl Default for DownloadQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadQueue {
    /// Create a new download queue.
    pub fn new() -> Self {
        Self {
            current_priority: AtomicU8::new(0),
            notify: Notify::new(),
        }
    }

    /// Check if a download is in progress.
    #[allow(dead_code)]
    pub fn is_downloading(&self) -> bool {
        self.current_priority.load(Ordering::SeqCst) > 0
    }

    /// Get the current download priority.
    #[allow(dead_code)]
    pub fn current_priority(&self) -> DownloadPriority {
        self.current_priority.load(Ordering::SeqCst).into()
    }

    /// Wait until no higher-priority download is in progress.
    ///
    /// Returns a guard that must be held during the download.
    /// When the guard is dropped, the download is marked as complete.
    pub async fn acquire(&self, priority: DownloadPriority) -> DownloadGuard<'_> {
        let priority_val = priority as u8;

        loop {
            let current = self.current_priority.load(Ordering::SeqCst);

            // If no download or lower priority download, we can proceed
            if current < priority_val {
                // Try to set our priority
                if self
                    .current_priority
                    .compare_exchange(current, priority_val, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    debug!(
                        "Download queue acquired (priority: {:?}, was: {:?})",
                        priority,
                        DownloadPriority::from(current)
                    );
                    return DownloadGuard {
                        queue: self,
                        priority: priority_val,
                    };
                }
                // CAS failed, retry
                continue;
            }

            // Higher or equal priority download in progress, wait
            if current >= priority_val {
                debug!(
                    "Waiting for higher priority download (current: {:?}, requested: {:?})",
                    DownloadPriority::from(current),
                    priority
                );
                self.notify.notified().await;
            }
        }
    }

    /// Try to acquire immediately without waiting.
    ///
    /// Returns None if a higher-priority download is in progress.
    #[allow(dead_code)]
    pub fn try_acquire(&self, priority: DownloadPriority) -> Option<DownloadGuard<'_>> {
        let priority_val = priority as u8;
        let current = self.current_priority.load(Ordering::SeqCst);

        if current < priority_val
            && self
                .current_priority
                .compare_exchange(current, priority_val, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            return Some(DownloadGuard {
                queue: self,
                priority: priority_val,
            });
        }
        None
    }

    /// Release a download slot (called by DownloadGuard on drop).
    fn release(&self, priority: u8) {
        // Only release if we still hold this priority
        let _ =
            self.current_priority
                .compare_exchange(priority, 0, Ordering::SeqCst, Ordering::SeqCst);
        self.notify.notify_waiters();
        debug!(
            "Download queue released (priority: {:?})",
            DownloadPriority::from(priority)
        );
    }
}

/// Guard that holds a download slot.
///
/// When dropped, releases the slot and notifies waiting downloads.
pub struct DownloadGuard<'a> {
    queue: &'a DownloadQueue,
    priority: u8,
}

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        self.queue.release(self.priority);
    }
}

/// Global download queue instance.
static DOWNLOAD_QUEUE: std::sync::OnceLock<Arc<DownloadQueue>> = std::sync::OnceLock::new();

/// Get the global download queue.
pub fn download_queue() -> Arc<DownloadQueue> {
    DOWNLOAD_QUEUE
        .get_or_init(|| Arc::new(DownloadQueue::new()))
        .clone()
}

/// Wait for higher priority downloads and acquire a slot.
pub async fn acquire_download_slot(priority: DownloadPriority) -> DownloadGuard<'static> {
    let queue = download_queue();
    // SAFETY: We're returning a guard with 'static lifetime because the queue is static
    // This is safe because DOWNLOAD_QUEUE lives for the entire program
    unsafe {
        std::mem::transmute::<DownloadGuard<'_>, DownloadGuard<'static>>(
            queue.acquire(priority).await,
        )
    }
}

/// Check if a download is currently in progress.
#[allow(dead_code)]
pub fn is_download_in_progress() -> bool {
    download_queue().is_downloading()
}

/// Log queue status for debugging.
#[allow(dead_code)]
pub fn log_queue_status() {
    let queue = download_queue();
    let priority = queue.current_priority();
    if priority != DownloadPriority::None {
        info!("Download in progress (priority: {:?})", priority);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(DownloadPriority::High > DownloadPriority::Medium);
        assert!(DownloadPriority::Medium > DownloadPriority::Low);
        assert!(DownloadPriority::Low > DownloadPriority::None);
    }

    #[test]
    fn test_priority_from_u8() {
        assert_eq!(DownloadPriority::from(0), DownloadPriority::None);
        assert_eq!(DownloadPriority::from(1), DownloadPriority::Low);
        assert_eq!(DownloadPriority::from(2), DownloadPriority::Medium);
        assert_eq!(DownloadPriority::from(3), DownloadPriority::High);
        assert_eq!(DownloadPriority::from(99), DownloadPriority::None);
    }

    #[test]
    fn test_queue_initial_state() {
        let queue = DownloadQueue::new();
        assert!(!queue.is_downloading());
        assert_eq!(queue.current_priority(), DownloadPriority::None);
    }

    #[test]
    fn test_try_acquire_empty_queue() {
        let queue = DownloadQueue::new();
        let guard = queue.try_acquire(DownloadPriority::Low);
        assert!(guard.is_some());
        assert!(queue.is_downloading());
        assert_eq!(queue.current_priority(), DownloadPriority::Low);
    }

    #[test]
    fn test_try_acquire_blocked_by_higher() {
        let queue = DownloadQueue::new();
        let _high = queue.try_acquire(DownloadPriority::High).unwrap();

        // Lower priority should fail
        let low = queue.try_acquire(DownloadPriority::Low);
        assert!(low.is_none());

        // Same priority should also fail
        let high2 = queue.try_acquire(DownloadPriority::High);
        assert!(high2.is_none());
    }

    #[test]
    fn test_guard_release() {
        let queue = DownloadQueue::new();
        {
            let _guard = queue.try_acquire(DownloadPriority::High).unwrap();
            assert!(queue.is_downloading());
        }
        // Guard dropped, queue should be empty
        assert!(!queue.is_downloading());
    }

    #[tokio::test]
    async fn test_acquire_waits_for_higher() {
        let queue = Arc::new(DownloadQueue::new());
        let queue2 = queue.clone();

        // Acquire high priority
        let high_guard = queue.try_acquire(DownloadPriority::High).unwrap();

        // Spawn task that waits for low priority
        let handle = tokio::spawn(async move {
            let _guard = queue2.acquire(DownloadPriority::Low).await;
            true
        });

        // Give the task time to start waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Drop high priority guard
        drop(high_guard);

        // Low priority should now complete
        let result = tokio::time::timeout(tokio::time::Duration::from_millis(100), handle).await;

        assert!(result.is_ok());
    }
}
