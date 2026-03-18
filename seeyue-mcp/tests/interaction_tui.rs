//! Integration tests: TUI state, theme, and render_tui module (P1-N5).
//!
//! These tests do NOT require a real terminal — they test state machine
//! logic and theme token generation without spawning a ratatui render loop.

use seeyue_mcp::interaction::{
    schema::{
        ColorProfile, CommentMode, InteractionKind, InteractionOption, InteractionRequest,
        InteractionStatus, PresentationHints, PresentationMode, SelectionMode,
    },
    state::{FocusPanel, TuiState},
    theme::Theme,
};

// ─── Helpers ────────────────────────────────────────────────────────────────

fn make_options(n: usize) -> Vec<InteractionOption> {
    (0..n)
        .map(|i| InteractionOption {
            id: format!("opt-{i}"),
            label: format!("Option {i}"),
            description: Some(format!("Description for option {i}")),
            recommended: i == 0,
            danger: i == n - 1 && n > 1,
            disabled: false,
            requires_comment: false,
            metadata: None,
        })
        .collect()
}

fn make_request(selection_mode: SelectionMode, comment_mode: CommentMode) -> InteractionRequest {
    InteractionRequest {
        schema: 1,
        interaction_id: "ix-20260318-001".to_string(),
        kind: InteractionKind::ApprovalRequest,
        status: InteractionStatus::Pending,
        title: "TUI 测试".to_string(),
        message: "请选择操作".to_string(),
        selection_mode,
        options: make_options(3),
        comment_mode,
        presentation: PresentationHints {
            mode: PresentationMode::TuiMenu,
            color_profile: ColorProfile::Auto,
            theme: "dark".to_string(),
            accent_token: None,
            show_details_by_default: false,
            allow_alternate_screen: true,
            keymap: None,
        },
        originating_request_id: "req-tui-001".to_string(),
        created_at: "2026-03-18T00:00:00Z".to_string(),
        detail: None,
        reason_code: None,
        risk_level: None,
        blocker_kind: None,
        default_option_ids: None,
        comment_label: None,
        comment_placeholder: None,
        recommended_next: None,
        origin: None,
        expires_at: None,
        timeout_seconds: None,
        scope: None,
    }
}

// ─── TuiState tests ─────────────────────────────────────────────────────────

#[test]
fn test_tui_state_initial_cursor_at_zero() {
    let opts = make_options(3);
    let state = TuiState::new(&opts, false);
    assert_eq!(state.cursor, 0);
    assert!(state.selected.is_empty());
    assert_eq!(state.focus, FocusPanel::Options);
}

#[test]
fn test_tui_state_cursor_down_wraps() {
    let opts = make_options(3);
    let mut state = TuiState::new(&opts, false);
    state.cursor = 2;
    state.cursor_down();
    assert_eq!(state.cursor, 0);
}

#[test]
fn test_tui_state_cursor_up_wraps() {
    let opts = make_options(3);
    let mut state = TuiState::new(&opts, false);
    state.cursor_up();
    assert_eq!(state.cursor, 2);
}

#[test]
fn test_tui_state_toggle_multi_select() {
    let opts = make_options(3);
    let mut state = TuiState::new(&opts, false);
    state.cursor = 1;
    state.toggle_select();
    assert!(state.is_selected(1));
    state.toggle_select();
    assert!(!state.is_selected(1));
}

#[test]
fn test_tui_state_confirm_single_replaces() {
    let opts = make_options(3);
    let mut state = TuiState::new(&opts, false);
    state.selected = vec![0];
    state.cursor = 2;
    state.confirm_single();
    assert_eq!(state.selected, vec![2]);
}

#[test]
fn test_tui_state_selected_ids() {
    let opts = make_options(3);
    let mut state = TuiState::new(&opts, false);
    state.selected = vec![0, 2];
    let ids = state.selected_ids(&opts);
    assert_eq!(ids, vec!["opt-0", "opt-2"]);
}

#[test]
fn test_tui_state_apply_defaults_single() {
    let opts = make_options(3);
    let mut state = TuiState::new(&opts, false);
    let defaults = vec!["opt-2".to_string()];
    state.apply_defaults(&opts, &defaults, &SelectionMode::SingleSelect);
    assert_eq!(state.cursor, 2);
    assert_eq!(state.selected, vec![2]);
}

// ─── Theme tests ─────────────────────────────────────────────────────────────

#[test]
fn test_theme_parse_variants() {
    assert_eq!(Theme::from_str("dark"), Theme::Dark);
    assert_eq!(Theme::from_str("light"), Theme::Light);
    assert_eq!(Theme::from_str("mono"), Theme::Mono);
    assert_eq!(Theme::from_str("auto"), Theme::Dark);
}

#[test]
fn test_theme_focused_differs_from_normal() {
    for theme in [Theme::Dark, Theme::Light, Theme::Mono] {
        assert_ne!(
            theme.focused_row(),
            theme.normal_row(),
            "focused_row must differ from normal_row for {theme:?}"
        );
    }
}

#[test]
fn test_theme_all_styles_compile() {
    for theme in [Theme::Dark, Theme::Light, Theme::Mono] {
        let _ = theme.focused_row();
        let _ = theme.normal_row();
        let _ = theme.selected_indicator();
        let _ = theme.danger_row();
        let _ = theme.title();
        let _ = theme.status_bar();
    }
}
