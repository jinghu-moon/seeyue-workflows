//! Integration tests: interaction CLI render and probe subcommands.

use seeyue_mcp::interaction::{
    cli::{self, RenderOptions, EXIT_PROCESS_ERROR, EXIT_VALIDATION_FAILED},
    io,
    schema::{
        CommentMode, InteractionKind, InteractionOption, InteractionRequest, InteractionStatus,
        PresentationHints, PresentationMode, SelectionMode,
    },
    terminal,
};
use tempfile::tempdir;

fn write_valid_request(path: &std::path::Path) {
    let req = InteractionRequest {
        schema: 1,
        interaction_id: "ix-20260318-001".to_string(),
        kind: InteractionKind::ApprovalRequest,
        status: InteractionStatus::Pending,
        title: "确认操作".to_string(),
        message: "请确认是否继续".to_string(),
        selection_mode: SelectionMode::SingleSelect,
        options: vec![InteractionOption {
            id: "approve".to_string(),
            label: "确认".to_string(),
            description: None,
            recommended: true,
            danger: false,
            disabled: false,
            requires_comment: false,
            metadata: None,
        }],
        comment_mode: CommentMode::Disabled,
        presentation: PresentationHints {
            mode: PresentationMode::TextMenu,
            color_profile: seeyue_mcp::interaction::schema::ColorProfile::Auto,
            theme: "auto".to_string(),
            accent_token: None,
            show_details_by_default: false,
            allow_alternate_screen: true,
            keymap: None,
        },
        originating_request_id: "req-123".to_string(),
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
    };
    let json = serde_json::to_string_pretty(&req).unwrap();
    std::fs::write(path, json).unwrap();
}

/// Missing request file → EXIT_PROCESS_ERROR (1)
#[test]
fn test_interaction_cli_invalid_request() {
    let dir = tempdir().unwrap();
    let req_path = dir.path().join("nonexistent.json");
    let resp_path = dir.path().join("response.json");
    let opts = RenderOptions {
        color_depth: seeyue_mcp::interaction::terminal::ColorDepth::Mono,
        theme: "auto".to_string(),
        no_alternate_screen: false,
        timeout_seconds: 0,
    };
    let code = cli::cmd_render(&req_path, &resp_path, "plain", opts);
    assert_eq!(code, EXIT_PROCESS_ERROR, "missing file must return EXIT_PROCESS_ERROR");
}

/// Request with invalid interaction_id → EXIT_VALIDATION_FAILED (3)
#[test]
fn test_interaction_cli_validation_failed() {
    let dir = tempdir().unwrap();
    let req_path = dir.path().join("bad.json");
    let resp_path = dir.path().join("response.json");
    let bad_req = serde_json::json!({
        "schema": 1,
        "interaction_id": "bad-id",
        "kind": "approval_request",
        "status": "pending",
        "title": "T",
        "message": "M",
        "selection_mode": "single_select",
        "options": [{"id": "ok", "label": "OK"}],
        "comment_mode": "disabled",
        "presentation": {
            "mode": "text_menu",
            "color_profile": "auto",
            "theme": "auto"
        },
        "originating_request_id": "req-1",
        "created_at": "2026-03-18T00:00:00Z"
    });
    std::fs::write(&req_path, serde_json::to_string_pretty(&bad_req).unwrap()).unwrap();
    let opts = RenderOptions {
        color_depth: seeyue_mcp::interaction::terminal::ColorDepth::Mono,
        theme: "auto".to_string(),
        no_alternate_screen: false,
        timeout_seconds: 0,
    };
    let code = cli::cmd_render(&req_path, &resp_path, "plain", opts);
    assert_eq!(code, EXIT_VALIDATION_FAILED, "bad id must return EXIT_VALIDATION_FAILED");
}

/// --mode tui always returns EXIT_TERMINAL_UNSUPPORTED in P0
#[test]
fn test_interaction_cli_tui_mode_returns_terminal_unsupported() {
    let dir = tempdir().unwrap();
    let req_path = dir.path().join("req.json");
    let resp_path = dir.path().join("response.json");
    write_valid_request(&req_path);
    let opts = RenderOptions {
        color_depth: seeyue_mcp::interaction::terminal::ColorDepth::Mono,
        theme: "auto".to_string(),
        no_alternate_screen: false,
        timeout_seconds: 0,
    };
    let code = cli::cmd_render(&req_path, &resp_path, "tui", opts);
    assert_eq!(code, cli::EXIT_TERMINAL_UNSUPPORTED, "--mode tui must return EXIT_TERMINAL_UNSUPPORTED in P0");
    // No response file should be written on terminal-unsupported exit
    assert!(!resp_path.exists(), "response file must not be written on EXIT_TERMINAL_UNSUPPORTED");
}

/// probe_terminal returns valid struct with expected fields
#[test]
fn test_interaction_cli_probe_terminal() {
    let caps = terminal::probe_terminal();
    // Serialization must succeed
    let json = serde_json::to_string(&caps).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v.get("is_tty").is_some());
    assert!(v.get("ansi_enabled").is_some());
    assert!(v.get("color_depth").is_some());
    assert!(v.get("supports_raw_mode").is_some());
    assert!(v.get("supports_alternate_screen").is_some());
    assert!(v.get("preferred_mode").is_some());
}

/// Exit code constants match spec values
#[test]
fn test_exit_codes() {
    assert_eq!(cli::EXIT_SUCCESS, 0);
    assert_eq!(cli::EXIT_PROCESS_ERROR, 1);
    assert_eq!(cli::EXIT_USER_CANCEL, 2);
    assert_eq!(cli::EXIT_VALIDATION_FAILED, 3);
    assert_eq!(cli::EXIT_TIMEOUT, 4);
    assert_eq!(cli::EXIT_TERMINAL_UNSUPPORTED, 5);
}

/// Reading a valid request file succeeds
#[test]
fn test_interaction_cli_read_request_ok() {
    let dir = tempdir().unwrap();
    let req_path = dir.path().join("req.json");
    write_valid_request(&req_path);
    let req = io::read_request(&req_path).unwrap();
    assert_eq!(req.interaction_id, "ix-20260318-001");
    assert_eq!(req.schema, 1);
}
