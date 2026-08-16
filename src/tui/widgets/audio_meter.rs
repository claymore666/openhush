//! Audio level meter widget using braille/block characters.

use crate::tui::theme::{audio_level_color, Theme};
use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

/// Block characters for simple bar display.
const BLOCK_CHARS: [char; 9] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '█'];

/// Audio level meter widget.
pub struct AudioMeter<'a> {
    /// Audio level history (0.0 to 1.0).
    levels: &'a [f32],
    /// Theme for colors.
    theme: Option<&'a Theme>,
}

impl<'a> AudioMeter<'a> {
    /// Create a new audio meter with the given level history.
    pub fn new(levels: &'a [f32]) -> Self {
        Self {
            levels,
            theme: None,
        }
    }

    /// Set the theme for colors.
    pub fn with_theme(mut self, theme: &'a Theme) -> Self {
        self.theme = Some(theme);
        self
    }
}

impl Widget for AudioMeter<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let width = area.width as usize;

        // Take the last `width` samples, or pad with zeros
        let samples: Vec<f32> = if self.levels.len() >= width {
            self.levels[self.levels.len() - width..].to_vec()
        } else {
            let mut padded = vec![0.0; width - self.levels.len()];
            padded.extend_from_slice(self.levels);
            padded
        };

        // Render each column
        for (x, &level) in samples.iter().enumerate() {
            let level = level.clamp(0.0, 1.0);

            // Choose color based on level and theme
            let color = if let Some(theme) = self.theme {
                audio_level_color(level, theme)
            } else {
                // Fallback colors if no theme
                if level > 0.8 {
                    ratatui::style::Color::Red
                } else if level > 0.5 {
                    ratatui::style::Color::Yellow
                } else {
                    ratatui::style::Color::Green
                }
            };

            // Choose character based on level
            let char_index = (level * 8.0) as usize;
            let ch = BLOCK_CHARS[char_index.min(8)];

            // Render at bottom of area
            let cell = buf.cell_mut((area.x + x as u16, area.y + area.height - 1));
            if let Some(cell) = cell {
                cell.set_char(ch);
                cell.set_style(Style::default().fg(color));
            }
        }
    }
}
