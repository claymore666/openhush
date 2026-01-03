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
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            // Only handle key press events, ignore release/repeat
                            if key.kind == KeyEventKind::Press
                                && sender_clone.send(Event::Key(key)).is_err()
                            {
                                return;
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            if sender_clone.send(Event::Mouse(mouse)).is_err() {
                                return;
                            }
                        }
                        Ok(CrosstermEvent::Resize(width, height)) => {
                            if sender_clone.send(Event::Resize(width, height)).is_err() {
                                return;
                            }
                        }
                        _ => {}
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
