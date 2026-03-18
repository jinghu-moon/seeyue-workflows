//! Theme tokens for sy-interact TUI.
//!
//! Provides a minimal colour palette derived from the terminal's colour depth
//! and the request's presentation.theme field.
//! P1: dark / light / mono themes. No external theme crate dependency.

use ratatui::style::{Color, Modifier, Style};

/// Named theme variants.
#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    Dark,
    Light,
    Mono,
}

impl Theme {
    /// Parse from CLI / request theme string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "light" => Self::Light,
            "mono" => Self::Mono,
            _ => Self::Dark, // "dark" or "auto" → dark
        }
    }

    /// Style for the focused/highlighted option row.
    pub fn focused_row(&self) -> Style {
        match self {
            Self::Dark => Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
            Self::Light => Style::default().fg(Color::White).bg(Color::Blue).add_modifier(Modifier::BOLD),
            Self::Mono => Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD),
        }
    }

    /// Style for a normal (unfocused) option row.
    pub fn normal_row(&self) -> Style {
        Style::default()
    }

    /// Style for a selected option indicator (multi-select checkmark).
    pub fn selected_indicator(&self) -> Style {
        match self {
            Self::Dark | Self::Light => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            Self::Mono => Style::default().add_modifier(Modifier::BOLD),
        }
    }

    /// Style for a danger option.
    pub fn danger_row(&self) -> Style {
        match self {
            Self::Dark | Self::Light => Style::default().fg(Color::Red),
            Self::Mono => Style::default().add_modifier(Modifier::DIM),
        }
    }

    /// Style for the title bar.
    pub fn title(&self) -> Style {
        match self {
            Self::Dark => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Self::Light => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            Self::Mono => Style::default().add_modifier(Modifier::BOLD),
        }
    }

    /// Style for the help/status bar at the bottom.
    pub fn status_bar(&self) -> Style {
        match self {
            Self::Dark => Style::default().fg(Color::DarkGray),
            Self::Light => Style::default().fg(Color::Gray),
            Self::Mono => Style::default().add_modifier(Modifier::DIM),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_str() {
        assert_eq!(Theme::from_str("dark"), Theme::Dark);
        assert_eq!(Theme::from_str("light"), Theme::Light);
        assert_eq!(Theme::from_str("mono"), Theme::Mono);
        assert_eq!(Theme::from_str("auto"), Theme::Dark);
        assert_eq!(Theme::from_str("unknown"), Theme::Dark);
    }

    #[test]
    fn test_focused_row_differs_from_normal() {
        for theme in [Theme::Dark, Theme::Light, Theme::Mono] {
            let focused = theme.focused_row();
            let normal = theme.normal_row();
            // At minimum, focused must differ from normal
            assert_ne!(focused, normal, "focused_row must differ from normal_row for {theme:?}");
        }
    }
}
