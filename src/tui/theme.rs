//! Color theme system for the TUI.
//!
//! Provides a centralized theme that can adapt to terminal capabilities
//! and respect user preferences (dark/light mode).

use ratatui::style::{Color, Modifier, Style};

/// Theme for the TUI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    /// Primary accent color (for active elements)
    pub accent: Color,
    /// Secondary accent color
    pub accent_dim: Color,
    /// Success/positive color
    pub success: Color,
    /// Warning color
    pub warning: Color,
    /// Error/danger color
    pub error: Color,
    /// Muted/disabled color
    pub muted: Color,
    /// Text color
    pub text: Color,
    /// Dim text color
    pub text_dim: Color,
    /// Border color for active panels
    pub border_active: Color,
    /// Border color for inactive panels
    pub border_inactive: Color,
    /// Background color (usually default terminal bg)
    pub bg: Color,
}

#[allow(dead_code)]
impl Theme {
    /// Create a theme that uses terminal default colors.
    ///
    /// This respects the user's terminal color scheme.
    pub fn terminal_default() -> Self {
        Self {
            accent: Color::Cyan,
            accent_dim: Color::DarkGray,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            muted: Color::DarkGray,
            text: Color::Reset, // Use terminal default
            text_dim: Color::DarkGray,
            border_active: Color::Cyan,
            border_inactive: Color::DarkGray,
            bg: Color::Reset, // Use terminal default
        }
    }

    /// Create a dark theme with explicit colors.
    #[allow(dead_code)]
    pub fn dark() -> Self {
        Self {
            accent: Color::Rgb(97, 175, 239),    // Light blue
            accent_dim: Color::Rgb(86, 95, 114), // Muted blue
            success: Color::Rgb(152, 195, 121),  // Green
            warning: Color::Rgb(229, 192, 123),  // Yellow
            error: Color::Rgb(224, 108, 117),    // Red
            muted: Color::Rgb(92, 99, 112),      // Gray
            text: Color::Rgb(171, 178, 191),     // Light gray
            text_dim: Color::Rgb(92, 99, 112),   // Dark gray
            border_active: Color::Rgb(97, 175, 239),
            border_inactive: Color::Rgb(62, 68, 81),
            bg: Color::Rgb(40, 44, 52), // Dark background
        }
    }

    /// Create a light theme with explicit colors.
    #[allow(dead_code)]
    pub fn light() -> Self {
        Self {
            accent: Color::Rgb(0, 122, 204), // Blue
            accent_dim: Color::Rgb(128, 128, 128),
            success: Color::Rgb(34, 139, 34), // Forest green
            warning: Color::Rgb(205, 133, 0), // Orange
            error: Color::Rgb(205, 49, 49),   // Red
            muted: Color::Rgb(160, 160, 160),
            text: Color::Rgb(51, 51, 51), // Dark gray
            text_dim: Color::Rgb(128, 128, 128),
            border_active: Color::Rgb(0, 122, 204),
            border_inactive: Color::Rgb(200, 200, 200),
            bg: Color::Rgb(255, 255, 255),
        }
    }

    // === Style helpers ===

    /// Style for normal text.
    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Style for dimmed/secondary text.
    pub fn text_dim_style(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    /// Style for accent/highlighted text.
    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Style for success indicators.
    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Style for warning indicators.
    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for error indicators.
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for muted/disabled elements.
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Style for active panel borders.
    pub fn border_active_style(&self) -> Style {
        Style::default().fg(self.border_active)
    }

    /// Style for inactive panel borders.
    pub fn border_inactive_style(&self) -> Style {
        Style::default().fg(self.border_inactive)
    }

    /// Style for selected/highlighted items.
    pub fn selected_style(&self) -> Style {
        Style::default()
            .fg(self.bg)
            .bg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for keyboard shortcut hints.
    pub fn shortcut_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for recording state indicator.
    pub fn recording_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for ready/idle state indicator.
    pub fn ready_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Style for processing state indicator.
    pub fn processing_style(&self) -> Style {
        Style::default().fg(self.warning)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::terminal_default()
    }
}

/// Get color for audio level (green -> yellow -> red).
pub fn audio_level_color(level: f32, theme: &Theme) -> Color {
    if level > 0.8 {
        theme.error
    } else if level > 0.5 {
        theme.warning
    } else {
        theme.success
    }
}
