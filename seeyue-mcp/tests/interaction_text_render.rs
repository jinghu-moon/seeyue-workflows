//! Integration tests: text_menu and plain_prompt renderers.

use seeyue_mcp::interaction::{
    renderer::{plain_prompt, text_menu, TerminalResponse},
    schema::{
        CommentMode, InteractionKind, InteractionOption, InteractionRequest, InteractionStatus,
        PresentationHints, PresentationMode, ResponseStatus, SelectionMode,
    },
};
use std::io::Cursor;

fn make_request(comment_mode: CommentMode) -> InteractionRequest {
    InteractionRequest {
        schema: 1,
        interaction_id: "ix-20260318-001".to_string(),
        kind: InteractionKind::ApprovalRequest,
        status: InteractionStatus::Pending,
        title: "确认操作".to_string(),
        message: "请确认是否继续".to_string(),
        selection_mode: SelectionMode::SingleSelect,
        options: vec![
            InteractionOption {
                id: "approve".to_string(),
                label: "确认".to_string(),
                description: None,
                recommended: true,
                danger: false,
                disabled: false,
                requires_comment: false,
                metadata: None,
            },
            InteractionOption {
                id: "deny".to_string(),
                label: "拒绝".to_string(),
                description: None,
                recommended: false,
                danger: false,
                disabled: false,
                requires_comment: false,
                metadata: None,
            },
        ],
        comment_mode,
        presentation: PresentationHints {
            mode: PresentationMode::TextMenu,
            color_profile: seeyue_mcp::interaction::schema::ColorProfile::Auto,
            theme: "auto".to_string(),
            accent_token: None,
            show_details_by_default: false,
            allow_alternate_screen: true,
            keymap: None,
        },
        originating_request_id: "req-1".to_string(),
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

/// text_menu with mock input returns Answered TerminalResponse
#[test]
fn test_text_menu_render_produces_response() {
    let req = make_request(CommentMode::Disabled);
    let options = req.options.clone();
    let mut stdin = Cursor::new("1\n");
    let mut stderr_buf = Vec::new();
    let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Answered);
    assert_eq!(resp.selected_option_ids, vec!["approve".to_string()]);
    assert_eq!(resp.presenter_mode, "text_menu");
    assert!(resp.comment.is_none());
}

/// text_menu selects second option
#[test]
fn test_text_menu_select_second_option() {
    let req = make_request(CommentMode::Disabled);
    let options = req.options.clone();
    let mut stdin = Cursor::new("2\n");
    let mut stderr_buf = Vec::new();
    let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Answered);
    assert_eq!(resp.selected_option_ids, vec!["deny".to_string()]);
}

/// text_menu cancels on 'q'
#[test]
fn test_text_menu_cancel_on_q() {
    let req = make_request(CommentMode::Disabled);
    let options = req.options.clone();
    let mut stdin = Cursor::new("q\n");
    let mut stderr_buf = Vec::new();
    let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Cancelled);
}

/// text_menu with optional comment captures it
#[test]
fn test_text_menu_with_optional_comment() {
    let req = make_request(CommentMode::Optional);
    let options = req.options.clone();
    let mut stdin = Cursor::new("1\nmy comment\n");
    let mut stderr_buf = Vec::new();
    let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Answered);
    assert_eq!(resp.comment, Some("my comment".to_string()));
}

/// plain_prompt returns TerminalResponse on 'y'
#[test]
fn test_plain_prompt_produces_response() {
    let req = make_request(CommentMode::Disabled);
    let mut stdin = Cursor::new("y\n");
    let mut stderr_buf = Vec::new();
    let resp = plain_prompt(&req, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Answered);
    assert_eq!(resp.presenter_mode, "plain_prompt");
    assert!(!resp.selected_option_ids.is_empty());
}

/// plain_prompt cancels on 'n'
#[test]
fn test_plain_prompt_cancel_on_n() {
    let req = make_request(CommentMode::Disabled);
    let mut stdin = Cursor::new("n\n");
    let mut stderr_buf = Vec::new();
    let resp = plain_prompt(&req, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Cancelled);
}

/// plain_prompt cancels on empty input (EOF)
#[test]
fn test_plain_prompt_empty_cancels() {
    let req = make_request(CommentMode::Disabled);
    let mut stdin = Cursor::new("");
    let mut stderr_buf = Vec::new();
    let resp = plain_prompt(&req, &mut stdin, &mut stderr_buf);
    assert_eq!(resp.status, ResponseStatus::Cancelled);
}

/// TerminalResponse::cancelled helper sets correct fields
#[test]
fn test_terminal_response_cancelled_helper() {
    let resp = TerminalResponse::cancelled("text_menu");
    assert_eq!(resp.status, ResponseStatus::Cancelled);
    assert!(resp.selected_option_ids.is_empty());
    assert!(resp.comment.is_none());
    assert_eq!(resp.presenter_mode, "text_menu");
}
