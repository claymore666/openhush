//! UI rendering for the TUI.

use crate::tui::app::{ActivePanel, App, RecordingState};
use crate::tui::widgets::audio_meter::AudioMeter;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Draw the entire UI.
pub fn draw(frame: &mut Frame, app: &App) {
    // Main layout: split into left (status) and right (transcription/history)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(frame.area());

    // Left panel: Status + Actions
    draw_left_panel(frame, app, main_chunks[0]);

    // Right panel: Transcription + History
    draw_right_panel(frame, app, main_chunks[1]);

    // Help overlay
    if app.show_help {
        draw_help_overlay(frame);
    }
}

/// Draw the left panel (status + actions).
fn draw_left_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Status
            Constraint::Length(4), // Audio meter
            Constraint::Length(4), // I/O summary
            Constraint::Min(0),    // Spacer
            Constraint::Length(5), // Actions
        ])
        .split(area);

    draw_status_panel(frame, app, chunks[0]);
    draw_audio_meter(frame, app, chunks[1]);
    draw_io_summary(frame, app, chunks[2]);
    draw_actions_panel(frame, chunks[4]);
}

/// Draw the status panel.
fn draw_status_panel(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == ActivePanel::Status;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Status ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Recording state with indicator
    let (state_icon, state_text, state_color) = match app.recording_state {
        RecordingState::Idle => ("○", "Ready", Color::Green),
        RecordingState::Recording => ("●", "Recording", Color::Red),
        RecordingState::Processing => ("◐", "Processing", Color::Yellow),
    };

    let duration_text = if app.recording_state == RecordingState::Recording {
        format!(" [{:.1}s]", app.recording_duration)
    } else {
        String::new()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(state_icon, Style::default().fg(state_color)),
            Span::raw(" "),
            Span::styled(state_text, Style::default().fg(state_color)),
            Span::styled(duration_text, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Model: "),
            Span::styled(&app.model_name, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Lang:  "),
            Span::styled(&app.language, Style::default().fg(Color::Cyan)),
            Span::raw(" → English"),
        ]),
        Line::from(vec![
            Span::raw("VAD:   "),
            Span::styled(
                if app.vad_enabled {
                    "● Active"
                } else {
                    "○ Off"
                },
                Style::default().fg(if app.vad_enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
        Line::from(vec![
            Span::raw("LLM:   "),
            Span::styled(
                if app.llm_enabled {
                    format!("● {}", app.llm_provider)
                } else {
                    "○ Off".to_string()
                },
                Style::default().fg(if app.llm_enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Draw the audio level meter.
fn draw_audio_meter(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Audio ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let meter = AudioMeter::new(&app.audio_history);
    frame.render_widget(meter, inner);
}

/// Draw I/O summary.
fn draw_io_summary(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::raw("In:  "),
            Span::styled("Default Mic", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Out: "),
            Span::styled("Clipboard ✓", Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::styled("Paste ✓", Style::default().fg(Color::Green)),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Draw the actions panel.
fn draw_actions_panel(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::styled("[r]", Style::default().fg(Color::Yellow)),
            Span::raw("ecord  "),
            Span::styled("[h]", Style::default().fg(Color::Yellow)),
            Span::raw("istory "),
            Span::styled("[m]", Style::default().fg(Color::Yellow)),
            Span::raw("odels"),
        ]),
        Line::from(vec![
            Span::styled("[c]", Style::default().fg(Color::Yellow)),
            Span::raw("onfig  "),
            Span::styled("[i]", Style::default().fg(Color::Yellow)),
            Span::raw("nput   "),
            Span::styled("[o]", Style::default().fg(Color::Yellow)),
            Span::raw("utput"),
        ]),
        Line::from(vec![
            Span::styled("[q]", Style::default().fg(Color::Yellow)),
            Span::raw("uit    "),
            Span::styled("[?]", Style::default().fg(Color::Yellow)),
            Span::raw("help"),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Draw the right panel (transcription + history).
fn draw_right_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_transcription_panel(frame, app, chunks[0]);
    draw_history_panel(frame, app, chunks[1]);
}

/// Draw the transcription panel.
fn draw_transcription_panel(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == ActivePanel::Transcription;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Transcription ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = if app.current_transcription.is_empty() {
        match app.recording_state {
            RecordingState::Idle => "Waiting for input...\n\nPress [r] to start recording",
            RecordingState::Recording => "Recording... speak now",
            RecordingState::Processing => "⟳ Transcribing...",
        }
        .to_string()
    } else {
        app.current_transcription.clone()
    };

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(
            Style::default().fg(if app.current_transcription.is_empty() {
                Color::DarkGray
            } else {
                Color::White
            }),
        );

    frame.render_widget(paragraph, inner);
}

/// Draw the history panel.
fn draw_history_panel(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == ActivePanel::History;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" History ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = app
        .history
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let style = if i == app.history_index && is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let content = Line::from(vec![
                Span::styled(
                    format!("{} ", entry.timestamp),
                    style.fg(if i == app.history_index && is_active {
                        Color::Black
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(truncate_string(&entry.text, 50), style),
            ]);

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

/// Draw help overlay.
fn draw_help_overlay(frame: &mut Frame) {
    let area = centered_rect(60, 70, frame.area());

    // Clear the area first
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab / Shift+Tab    Switch panels"),
        Line::from("  ↑/↓ or j/k         Navigate history"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Recording",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  r                  Start/stop recording"),
        Line::from("  s                  Stop recording"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Panels",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  h                  Open history"),
        Line::from("  m                  Open model manager"),
        Line::from("  c                  Open config"),
        Line::from("  i                  Open input selector"),
        Line::from("  o                  Open output config"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ?                  Toggle this help"),
        Line::from("  q                  Quit"),
        Line::from("  Ctrl+C             Quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press Esc or ? to close",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let paragraph = Paragraph::new(help_text);
    frame.render_widget(paragraph, inner);
}

/// Create a centered rect.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Truncate a string to a maximum length.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
