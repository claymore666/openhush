//! Terminal User Interface (TUI) for OpenHush.
//!
//! A lazygit-style panel interface for controlling OpenHush from the terminal.
//! Features real-time audio levels, transcription display, and keyboard-driven navigation.

mod app;
mod event;
mod ui;
pub mod widgets;

pub use app::{App, AppResult};
pub use event::{Event, EventHandler};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, stdout};
use tracing::{debug, error, info};

/// Run the TUI application.
pub fn run() -> AppResult<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    info!("Starting OpenHush TUI");

    // Create app and run it
    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        error!("TUI error: {}", e);
    }

    result
}

/// Main application loop.
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> AppResult<()> {
    let event_handler = EventHandler::new(250); // 250ms tick rate

    while app.is_running() {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle events
        match event_handler.next()? {
            Event::Tick => {
                app.on_tick();
            }
            Event::Key(key_event) => {
                app.on_key(key_event);
            }
            Event::Mouse(mouse_event) => {
                app.on_mouse(mouse_event);
            }
            Event::Resize(width, height) => {
                debug!("Terminal resized to {}x{}", width, height);
            }
        }
    }

    Ok(())
}
