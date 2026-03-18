//! TUI state types for sy-interact.
//!
//! Holds focus, selection, and comment buffer state for the ratatui renderer.
//! P1: single/multi-select, comment input, details panel toggle.

use crate::interaction::schema::{InteractionOption, SelectionMode};

/// Which UI panel has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusPanel {
    /// Option list (default)
    Options,
    /// Comment text input
    Comment,
}

/// Mutable TUI state threaded through the render loop.
#[derive(Debug, Clone)]
pub struct TuiState {
    /// Index of the highlighted option (0-based).
    pub cursor: usize,
    /// Selected option indices (for multi-select).
    pub selected: Vec<usize>,
    /// Comment buffer (for optional/required comment modes).
    pub comment: String,
    /// Which panel currently holds keyboard focus.
    pub focus: FocusPanel,
    /// Whether the details panel is expanded.
    pub show_details: bool,
    /// Number of options available.
    pub option_count: usize,
}

impl TuiState {
    /// Create initial state from options and selection mode.
    pub fn new(options: &[InteractionOption], show_details: bool) -> Self {
        Self {
            cursor: 0,
            selected: Vec::new(),
            comment: String::new(),
            focus: FocusPanel::Options,
            show_details,
            option_count: options.len(),
        }
    }

    /// Move cursor up (wraps).
    pub fn cursor_up(&mut self) {
        if self.option_count == 0 { return; }
        self.cursor = if self.cursor == 0 {
            self.option_count - 1
        } else {
            self.cursor - 1
        };
    }

    /// Move cursor down (wraps).
    pub fn cursor_down(&mut self) {
        if self.option_count == 0 { return; }
        self.cursor = (self.cursor + 1) % self.option_count;
    }

    /// Toggle selection of the current cursor option (multi-select).
    pub fn toggle_select(&mut self) {
        if let Some(pos) = self.selected.iter().position(|&i| i == self.cursor) {
            self.selected.remove(pos);
        } else {
            self.selected.push(self.cursor);
        }
    }

    /// Confirm selection for single-select: replace selected with cursor.
    pub fn confirm_single(&mut self) {
        self.selected = vec![self.cursor];
    }

    /// Whether the current cursor option is selected (multi-select).
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// Get selected option IDs from the options slice.
    pub fn selected_ids<'a>(&self, options: &'a [InteractionOption]) -> Vec<&'a str> {
        self.selected
            .iter()
            .filter_map(|&i| options.get(i))
            .map(|o| o.id.as_str())
            .collect()
    }

    /// Apply default selection from pre-selected IDs.
    pub fn apply_defaults(&mut self, options: &[InteractionOption], default_ids: &[String], mode: &SelectionMode) {
        for (i, opt) in options.iter().enumerate() {
            if default_ids.contains(&opt.id) {
                match mode {
                    SelectionMode::MultiSelect => { self.selected.push(i); }
                    _ => { self.selected = vec![i]; self.cursor = i; break; }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::schema::{
        ColorProfile, CommentMode, InteractionKind, InteractionOption, InteractionRequest,
        InteractionStatus, PresentationHints, PresentationMode, SelectionMode,
    };

    fn make_options(n: usize) -> Vec<InteractionOption> {
        (0..n).map(|i| InteractionOption {
            id: format!("opt-{i}"),
            label: format!("Option {i}"),
            description: None,
            recommended: i == 0,
            danger: false,
            disabled: false,
            requires_comment: false,
            metadata: None,
        }).collect()
    }

    #[test]
    fn test_cursor_wrap_down() {
        let opts = make_options(3);
        let mut state = TuiState::new(&opts, false);
        state.cursor = 2;
        state.cursor_down();
        assert_eq!(state.cursor, 0, "cursor wraps from last to first");
    }

    #[test]
    fn test_cursor_wrap_up() {
        let opts = make_options(3);
        let mut state = TuiState::new(&opts, false);
        state.cursor = 0;
        state.cursor_up();
        assert_eq!(state.cursor, 2, "cursor wraps from first to last");
    }

    #[test]
    fn test_toggle_select_add_remove() {
        let opts = make_options(3);
        let mut state = TuiState::new(&opts, false);
        state.cursor = 1;
        state.toggle_select();
        assert!(state.is_selected(1));
        state.toggle_select();
        assert!(!state.is_selected(1));
    }

    #[test]
    fn test_confirm_single_replaces() {
        let opts = make_options(3);
        let mut state = TuiState::new(&opts, false);
        state.selected = vec![0];
        state.cursor = 2;
        state.confirm_single();
        assert_eq!(state.selected, vec![2]);
    }

    #[test]
    fn test_selected_ids() {
        let opts = make_options(3);
        let mut state = TuiState::new(&opts, false);
        state.selected = vec![0, 2];
        let ids = state.selected_ids(&opts);
        assert_eq!(ids, vec!["opt-0", "opt-2"]);
    }
}
