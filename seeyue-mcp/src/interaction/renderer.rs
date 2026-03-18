//! Fallback renderers for sy-interact.
//!
//! Two renderers that do not require raw mode or alternate screen:
//! - `text_menu`: numbered list printed to stderr, reads line from stdin
//! - `plain_prompt`: simple y/n or free-text prompt to stderr, reads from stdin
//!
//! Both write display to stderr, read input from stdin.
//! Both return a TerminalResponse the caller converts to InteractionResponse.
//! No ratatui dependency — P0 fallback only.

use std::io::{BufRead, Write};
use crate::interaction::schema::{CommentMode, InteractionOption, InteractionRequest, ResponseStatus};

/// The result returned by a renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalResponse {
    pub status: ResponseStatus,
    pub selected_option_ids: Vec<String>,
    pub comment: Option<String>,
    pub presenter_mode: String,
}

impl TerminalResponse {
    pub fn cancelled(presenter_mode: &str) -> Self {
        Self {
            status: ResponseStatus::Cancelled,
            selected_option_ids: Vec::new(),
            comment: None,
            presenter_mode: presenter_mode.to_string(),
        }
    }
}

/// Render a numbered text menu to stderr, read choice from stdin.
///
/// `stdin` is injected for testing; pass real stdin wrapped in BufReader for production.
pub fn text_menu<R: BufRead, W: Write>(
    request: &InteractionRequest,
    options: &[InteractionOption],
    stdin: &mut R,
    stderr: &mut W,
) -> TerminalResponse {
    // Print header
    writeln!(stderr, "\n[sy-interact] {}", request.title).ok();
    writeln!(stderr, "{}", request.message).ok();
    writeln!(stderr).ok();

    // Print numbered options
    for (i, opt) in options.iter().enumerate() {
        let rec = if opt.recommended { " (recommended)" } else { "" };
        let danger = if opt.danger { " [!]" } else { "" };
        writeln!(stderr, "  {}){danger} {}{rec}", i + 1, opt.label).ok();
    }

    // Optional comment prompt
    let needs_comment = matches!(
        request.comment_mode,
        CommentMode::Required | CommentMode::Optional
    );

    loop {
        write!(stderr, "\nEnter choice (1-{}): ", options.len()).ok();
        stderr.flush().ok();

        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => return TerminalResponse::cancelled("text_menu"), // EOF
            Ok(_) => {}
            Err(_) => return TerminalResponse::cancelled("text_menu"),
        }

        let trimmed = line.trim();
        // Allow cancel via empty or 'q'
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("q") {
            return TerminalResponse::cancelled("text_menu");
        }

        if let Ok(n) = trimmed.parse::<usize>() {
            if n >= 1 && n <= options.len() {
                let selected = options[n - 1].clone();
                let comment = if needs_comment {
                    read_comment(stdin, stderr, &request.comment_mode)
                } else {
                    None
                };
                return TerminalResponse {
                    status: ResponseStatus::Answered,
                    selected_option_ids: vec![selected.id],
                    comment,
                    presenter_mode: "text_menu".to_string(),
                };
            }
        }

        writeln!(stderr, "  Invalid choice. Please enter 1-{}.", options.len()).ok();
    }
}

/// Render a simple y/n prompt to stderr, read response from stdin.
///
/// Used when neither TUI nor text menu is appropriate (non-TTY fallback).
pub fn plain_prompt<R: BufRead, W: Write>(
    request: &InteractionRequest,
    stdin: &mut R,
    stderr: &mut W,
) -> TerminalResponse {
    writeln!(stderr, "\n[sy-interact] {}", request.title).ok();
    writeln!(stderr, "{}", request.message).ok();

    // Show first option as positive, any 'n' as cancel
    if let Some(first_opt) = request.options.first() {
        writeln!(stderr, "Options: {}", request.options.iter()
            .map(|o| o.label.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
        ).ok();
        writeln!(stderr, "Enter option number (1-{}) or 'n' to cancel: ", request.options.len()).ok();
        stderr.flush().ok();

        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => return TerminalResponse::cancelled("plain_prompt"),
            Ok(_) => {}
            Err(_) => return TerminalResponse::cancelled("plain_prompt"),
        }

        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("n")
            || trimmed.eq_ignore_ascii_case("no")
            || trimmed.is_empty()
        {
            return TerminalResponse::cancelled("plain_prompt");
        }

        if let Ok(n) = trimmed.parse::<usize>() {
            if n >= 1 && n <= request.options.len() {
                let selected = request.options[n - 1].clone();
                return TerminalResponse {
                    status: ResponseStatus::Answered,
                    selected_option_ids: vec![selected.id],
                    comment: None,
                    presenter_mode: "plain_prompt".to_string(),
                };
            }
        }

        // y/yes maps to first option
        if trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes") {
            return TerminalResponse {
                status: ResponseStatus::Answered,
                selected_option_ids: vec![first_opt.id.clone()],
                comment: None,
                presenter_mode: "plain_prompt".to_string(),
            };
        }
    }

    TerminalResponse::cancelled("plain_prompt")
}

/// Read an optional/required comment from stdin.
fn read_comment<R: BufRead, W: Write>(
    stdin: &mut R,
    stderr: &mut W,
    mode: &CommentMode,
) -> Option<String> {
    let prompt = match mode {
        CommentMode::Required => "Comment (required): ",
        CommentMode::Optional => "Comment (optional, press Enter to skip): ",
        CommentMode::Disabled => return None,
    };
    write!(stderr, "{prompt}").ok();
    stderr.flush().ok();

    let mut line = String::new();
    if stdin.read_line(&mut line).is_ok() {
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::schema::{
        CommentMode, InteractionKind, InteractionOption, InteractionRequest, InteractionStatus,
        PresentationHints, PresentationMode, SelectionMode,
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
                color_profile: crate::interaction::schema::ColorProfile::Auto,
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
    }

    #[test]
    fn test_text_menu_cancel_on_q() {
        let req = make_request(CommentMode::Disabled);
        let options = req.options.clone();
        let mut stdin = Cursor::new("q\n");
        let mut stderr_buf = Vec::new();
        let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
        assert_eq!(resp.status, ResponseStatus::Cancelled);
    }

    #[test]
    fn test_text_menu_cancel_on_eof() {
        let req = make_request(CommentMode::Disabled);
        let options = req.options.clone();
        let mut stdin = Cursor::new("");
        let mut stderr_buf = Vec::new();
        let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
        assert_eq!(resp.status, ResponseStatus::Cancelled);
    }

    #[test]
    fn test_text_menu_with_comment() {
        let req = make_request(CommentMode::Optional);
        let options = req.options.clone();
        // Select option 1, then provide comment
        let mut stdin = Cursor::new("1\nmy comment\n");
        let mut stderr_buf = Vec::new();
        let resp = text_menu(&req, &options, &mut stdin, &mut stderr_buf);
        assert_eq!(resp.status, ResponseStatus::Answered);
        assert_eq!(resp.comment, Some("my comment".to_string()));
    }

    #[test]
    fn test_plain_prompt_produces_response() {
        let req = make_request(CommentMode::Disabled);
        let mut stdin = Cursor::new("y\n");
        let mut stderr_buf = Vec::new();
        let resp = plain_prompt(&req, &mut stdin, &mut stderr_buf);
        assert_eq!(resp.status, ResponseStatus::Answered);
        assert_eq!(resp.presenter_mode, "plain_prompt");
    }

    #[test]
    fn test_plain_prompt_cancel_on_n() {
        let req = make_request(CommentMode::Disabled);
        let mut stdin = Cursor::new("n\n");
        let mut stderr_buf = Vec::new();
        let resp = plain_prompt(&req, &mut stdin, &mut stderr_buf);
        assert_eq!(resp.status, ResponseStatus::Cancelled);
    }

    #[test]
    fn test_plain_prompt_empty_input_cancels() {
        let req = make_request(CommentMode::Disabled);
        let mut stdin = Cursor::new("");
        let mut stderr_buf = Vec::new();
        let resp = plain_prompt(&req, &mut stdin, &mut stderr_buf);
        assert_eq!(resp.status, ResponseStatus::Cancelled);
    }

    #[test]
    fn test_text_menu_output_contains_title() {
        let req = make_request(CommentMode::Disabled);
        let options = req.options.clone();
        let mut stdin = Cursor::new("1\n");
        let mut stderr_buf = Vec::new();
        text_menu(&req, &options, &mut stdin, &mut stderr_buf);
        let output = String::from_utf8_lossy(&stderr_buf);
        assert!(output.contains("确认操作"), "stderr should contain title");
        assert!(output.contains("确认"), "stderr should contain option label");
    }
}
