//! Audio level meter widget using braille/block characters.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

/// Braille characters for audio visualization.
/// Each braille character can represent 2x4 dots (2 columns, 4 rows).
const BRAILLE_PATTERNS: [char; 9] = [
    ' ', '⣀', '⣤', '⣶', '⣿', // Bottom to top fill
    '▁', '▂', '▃', '▄',
];

/// Block characters for simple bar display.
const BLOCK_CHARS: [char; 9] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '█'];

/// Audio level meter widget.
pub struct AudioMeter<'a> {
    /// Audio level history (0.0 to 1.0).
    levels: &'a [f32],
    /// Use braille characters (more resolution) vs blocks.
    use_braille: bool,
}

impl<'a> AudioMeter<'a> {
    /// Create a new audio meter with the given level history.
    pub fn new(levels: &'a [f32]) -> Self {
        Self {
            levels,
            use_braille: false,
        }
    }

    /// Use braille characters for higher resolution.
    #[allow(dead_code)]
    pub fn braille(mut self) -> Self {
        self.use_braille = true;
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

            // Choose color based on level
            let color = if level > 0.8 {
                Color::Red
            } else if level > 0.5 {
                Color::Yellow
            } else {
                Color::Green
            };

            // Choose character based on level
            let char_index = (level * 8.0) as usize;
            let ch = if self.use_braille {
                BRAILLE_PATTERNS[char_index.min(4)]
            } else {
                BLOCK_CHARS[char_index.min(8)]
            };

            // Render at bottom of area
            let cell = buf.cell_mut((area.x + x as u16, area.y + area.height - 1));
            if let Some(cell) = cell {
                cell.set_char(ch);
                cell.set_style(Style::default().fg(color));
            }
        }
    }
}
