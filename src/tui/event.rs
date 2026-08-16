//! Event handling for the TUI.

use crate::tui::AppResult;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Terminal events.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// Terminal tick (for animations/updates).
    Tick,
    /// Key press.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Terminal resize.
    Resize(u16, u16),
}

/// Handles terminal events in a separate thread.
pub struct EventHandler {
    /// Event receiver channel.
    receiver: mpsc::Receiver<Event>,
    /// Event sender (kept for potential future use).
    #[allow(dead_code)]
    sender: mpsc::Sender<Event>,
}

impl EventHandler {
    /// Create a new event handler with the given tick rate in milliseconds.
    pub fn new(tick_rate_ms: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate_ms);
        let (sender, receiver) = mpsc::channel();
        let sender_clone = sender.clone();

        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                // Calculate timeout until next tick
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);

                // Poll for events with timeout
                if event::poll(timeout).unwrap_or(false) {
                    let event = match event::read() {
                        // Only handle key press events, ignore release/repeat
                        Ok(CrosstermEvent::Key(key)) if key.kind == KeyEventKind::Press => {
                            Some(Event::Key(key))
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => Some(Event::Mouse(mouse)),
                        Ok(CrosstermEvent::Resize(width, height)) => {
                            Some(Event::Resize(width, height))
                        }
                        _ => None,
                    };

                    // An error means the receiver is gone, so stop the loop.
                    if let Some(event) = event {
                        if sender_clone.send(event).is_err() {
                            return;
                        }
                    }
                }

                // Send tick if enough time has passed
                if last_tick.elapsed() >= tick_rate {
                    if sender_clone.send(Event::Tick).is_err() {
                        return;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self { receiver, sender }
    }

    /// Get the next event, blocking until one is available.
    pub fn next(&self) -> AppResult<Event> {
        Ok(self.receiver.recv()?)
    }
}
