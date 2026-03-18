//! CLI logic for sy-interact.
//!
//! Exit codes (docs/sy-interact-cli-spec.md section 6):
//!   0 = success (response written, including valid denial)
//!   1 = process error (bad args, missing file, IO error)
//!   2 = user cancelled (Ctrl-C / ESC / 'q')
//!   3 = request validation failed
//!   4 = timeout
//!   5 = terminal unsupported, no fallback

use clap::{Parser, Subcommand};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use crate::interaction::{
    io,
    renderer,
    schema::{validate_request, InteractionResponse, PresentationMode, ResponseStatus},
    terminal::{self, ColorDepth},
};

/// Resolved render options passed through the execution path.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub color_depth: ColorDepth,
    pub theme: String,
    pub no_alternate_screen: bool,
    pub timeout_seconds: u32,
}

// ─── Exit codes ────────────────────────────────────────────────────────────

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_PROCESS_ERROR: i32 = 1;
pub const EXIT_USER_CANCEL: i32 = 2;
pub const EXIT_VALIDATION_FAILED: i32 = 3;
pub const EXIT_TIMEOUT: i32 = 4;
pub const EXIT_TERMINAL_UNSUPPORTED: i32 = 5;

// ─── CLI definition ────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "sy-interact",
    version,
    about = "Presenter-only interaction renderer for seeyue-workflows",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Read a request file, render interaction, write response file
    Render {
        /// Path to the interaction request JSON file
        #[arg(long = "request-file", value_name = "PATH")]
        request_file: PathBuf,

        /// Path to write the interaction response JSON file
        #[arg(long = "response-file", value_name = "PATH")]
        response_file: PathBuf,

        /// Color mode: auto, always, never
        #[arg(long, default_value = "auto")]
        color: String,

        /// Color depth: auto, 16, 256, 24
        #[arg(long = "color-depth", default_value = "auto")]
        color_depth: String,

        /// Theme: auto, dark, light
        #[arg(long, default_value = "auto")]
        theme: String,

        /// Rendering mode: auto, tui, text, plain
        #[arg(long, default_value = "auto")]
        mode: String,

        /// Timeout in seconds (0 = no timeout)
        #[arg(long = "timeout-seconds", default_value = "0")]
        timeout_seconds: u32,

        /// Disable alternate screen (use inline rendering)
        #[arg(long = "no-alternate-screen")]
        no_alternate_screen: bool,
    },

    /// Probe terminal capabilities and print result
    ProbeTerminal {
        /// Output format: json, text
        #[arg(long, default_value = "json")]
        format: String,
    },
}

/// Entry point called from main().
pub fn main() {
    let cli = Cli::parse();
    let code = run(cli);
    std::process::exit(code);
}

/// Run the parsed CLI, return exit code.
pub fn run(cli: Cli) -> i32 {
    match cli.command {
        Commands::Render {
            request_file,
            response_file,
            color: _,
            color_depth,
            theme,
            mode,
            timeout_seconds,
            no_alternate_screen,
        } => {
            let opts = RenderOptions {
                color_depth: resolve_color_depth(&color_depth),
                theme,
                no_alternate_screen,
                timeout_seconds,
            };
            cmd_render(&request_file, &response_file, &mode, opts)
        }

        Commands::ProbeTerminal { format } => cmd_probe_terminal(&format),
    }
}

/// Execute the render subcommand.
pub fn cmd_render(
    request_file: &std::path::Path,
    response_file: &std::path::Path,
    mode: &str,
    opts: RenderOptions,
) -> i32 {
    // 1. Read request file
    let request = match io::read_request(request_file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[sy-interact] error reading request: {e}");
            return EXIT_PROCESS_ERROR;
        }
    };

    // 2. Validate request
    if let Err(e) = validate_request(&request) {
        eprintln!("[sy-interact] request validation failed: {e}");
        return EXIT_VALIDATION_FAILED;
    }

    // 3. Probe terminal capabilities
    let caps = terminal::probe_terminal();

    // 3a. Resolve effective color depth:
    //     CLI "auto" → use probed result; explicit value → use as-is
    let effective_color_depth = resolve_effective_color_depth(&opts.color_depth, &caps);

    // 3b. Apply no_alternate_screen constraint to caps
    let effective_caps = if opts.no_alternate_screen {
        let mut c = caps.clone();
        c.supports_alternate_screen = false;
        // Without alternate screen, prefer text mode over tui
        if c.preferred_mode == "tui" {
            c.preferred_mode = "text".to_string();
        }
        c
    } else {
        caps.clone()
    };

    // 4. Resolve effective mode.
    //    "tui" is NOT implemented in P0 — always reject with EXIT_TERMINAL_UNSUPPORTED.
    //    This keeps the host wrapper contract stable: --mode tui never silently degrades.
    if mode == "tui" {
        eprintln!("[sy-interact] tui mode is not implemented in P0; use 'text' or 'plain'");
        return EXIT_TERMINAL_UNSUPPORTED;
    }

    let effective_mode = resolve_mode(mode, &effective_caps);

    // 6. Render — with optional timeout via thread + channel
    let options = request.options.clone();
    let terminal_resp = if opts.timeout_seconds > 0 {
        let (tx, rx) = mpsc::channel();
        let req_clone = request.clone();
        let opts_clone = options.clone();
        let mode_str = effective_mode.clone();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin.lock());
            let mut stderr = std::io::stderr();
            let resp = match mode_str.as_str() {
                "text" => renderer::text_menu(&req_clone, &opts_clone, &mut reader, &mut stderr),
                _ => renderer::plain_prompt(&req_clone, &mut reader, &mut stderr),
            };
            let _ = tx.send(resp);
        });
        match rx.recv_timeout(Duration::from_secs(opts.timeout_seconds as u64)) {
            Ok(resp) => resp,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                eprintln!("[sy-interact] timeout after {}s", opts.timeout_seconds);
                renderer::TerminalResponse {
                    status: ResponseStatus::Timeout,
                    selected_option_ids: vec![],
                    comment: None,
                    presenter_mode: effective_mode.clone(),
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => renderer::TerminalResponse::cancelled(&effective_mode),
        }
    } else {
        let stdin = std::io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let mut stderr = std::io::stderr();
        match effective_mode.as_str() {
            "text" => renderer::text_menu(&request, &options, &mut reader, &mut stderr),
            _ => renderer::plain_prompt(&request, &mut reader, &mut stderr),
        }
    };

    // 7. Map TerminalResponse → InteractionResponse
    // presenter.mode reflects the ACTUAL renderer used (never tui_menu in P0)
    let pmode = mode_to_presentation_mode(&effective_mode);

    let mut response = InteractionResponse::new(
        &request.interaction_id,
        terminal_resp.status.clone(),
        selection_mode_to_str(&request.selection_mode),
        pmode,
    );
    // Populate presenter fields from actual probed/resolved values
    response.presenter.color_depth = Some(effective_color_depth);
    // terminal_kind: not populated in P0 (requires OS-level terminal detection)
    response.selected_option_ids = terminal_resp.selected_option_ids;
    response.comment = terminal_resp.comment;

    // 8. Write response
    if let Err(e) = io::write_response(response_file, &response) {
        eprintln!("[sy-interact] error writing response: {e}");
        return EXIT_PROCESS_ERROR;
    }

    // 9. Return exit code based on status
    match &response.status {
        ResponseStatus::Answered => EXIT_SUCCESS,
        ResponseStatus::Cancelled => EXIT_USER_CANCEL,
        ResponseStatus::Timeout => EXIT_TIMEOUT,
        ResponseStatus::Failed => EXIT_PROCESS_ERROR,
    }
}

// Suppress unused warning — caps is used via effective_caps
#[allow(unused_variables)]
fn _use_caps(caps: terminal::TerminalCapabilities) {}

/// Execute the probe-terminal subcommand.
pub fn cmd_probe_terminal(format: &str) -> i32 {
    let caps = terminal::probe_terminal();
    match format {
        "text" => {
            println!("is_tty: {}", caps.is_tty);
            println!("ansi_enabled: {}", caps.ansi_enabled);
            println!("color_depth: {}", serde_json::to_string(&caps.color_depth).unwrap_or_default().trim_matches('"'));
            println!("supports_raw_mode: {}", caps.supports_raw_mode);
            println!("supports_alternate_screen: {}", caps.supports_alternate_screen);
            println!("preferred_mode: {}", caps.preferred_mode);
            println!("columns: {}", caps.columns);
            println!("rows: {}", caps.rows);
        }
        _ => {
            // json (default)
            match serde_json::to_string_pretty(&caps) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("[sy-interact] serialization error: {e}");
                    return EXIT_PROCESS_ERROR;
                }
            }
        }
    }
    EXIT_SUCCESS
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn resolve_mode(mode: &str, caps: &terminal::TerminalCapabilities) -> String {
    match mode {
        "auto" => caps.preferred_mode.clone(),
        other => other.to_string(),
    }
}

fn mode_to_presentation_mode(mode: &str) -> PresentationMode {
    match mode {
        "tui" => PresentationMode::TuiMenu,
        "text" => PresentationMode::TextMenu,
        _ => PresentationMode::PlainPrompt,
    }
}

fn selection_mode_to_str(mode: &crate::interaction::schema::SelectionMode) -> &'static str {
    use crate::interaction::schema::SelectionMode;
    match mode {
        SelectionMode::Boolean => "boolean",
        SelectionMode::SingleSelect => "single_select",
        SelectionMode::MultiSelect => "multi_select",
        SelectionMode::Text => "text",
        SelectionMode::Number => "number",
        SelectionMode::Path => "path",
        SelectionMode::Secret => "secret",
    }
}

/// Parse CLI color-depth string into ColorDepth enum.
/// "auto" is a sentinel — caller must call resolve_effective_color_depth to get real value.
fn resolve_color_depth(s: &str) -> ColorDepth {
    match s {
        "16" | "ansi16" => ColorDepth::Ansi16,
        "256" | "ansi256" => ColorDepth::Ansi256,
        "24" | "truecolor" | "true_color" => ColorDepth::TrueColor,
        _ => ColorDepth::Mono, // sentinel for "auto" — replaced by probe result
    }
}

/// Resolve the effective color depth:
/// - If CLI was "auto" (stored as Mono sentinel), use the probed terminal value.
/// - Otherwise, use the explicit CLI override.
fn resolve_effective_color_depth(
    cli_depth: &ColorDepth,
    caps: &terminal::TerminalCapabilities,
) -> ColorDepth {
    // Mono is used as the "auto" sentinel (CLI default_value = "auto" → resolve_color_depth → Mono)
    // We distinguish auto from explicit mono by checking the raw CLI string is not needed here:
    // The contract: if user passed "auto", we probe; if they passed "16"/"256"/"24", we use that.
    // Since "auto" → Mono sentinel and explicit "mono" is not a valid CLI value,
    // Mono here always means "use probe result".
    match cli_depth {
        ColorDepth::Mono => caps.color_depth.clone(),
        explicit => explicit.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::schema::{
        ColorProfile, CommentMode, InteractionKind, InteractionOption, InteractionRequest,
        InteractionStatus, PresentationHints, PresentationMode, SelectionMode,
    };
    use tempfile::tempdir;

    pub fn write_sample_request(path: &std::path::Path) {
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
                color_profile: ColorProfile::Auto,
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

    #[test]
    fn test_exit_codes() {
        assert_eq!(EXIT_SUCCESS, 0);
        assert_eq!(EXIT_PROCESS_ERROR, 1);
        assert_eq!(EXIT_USER_CANCEL, 2);
        assert_eq!(EXIT_VALIDATION_FAILED, 3);
        assert_eq!(EXIT_TIMEOUT, 4);
        assert_eq!(EXIT_TERMINAL_UNSUPPORTED, 5);
    }

    #[test]
    fn test_cmd_render_missing_file_returns_process_error() {
        let dir = tempdir().unwrap();
        let req_path = dir.path().join("nonexistent.json");
        let resp_path = dir.path().join("response.json");
        let opts = RenderOptions {
            color_depth: crate::interaction::terminal::ColorDepth::Mono,
            theme: "auto".to_string(),
            no_alternate_screen: false,
            timeout_seconds: 0,
        };
        let code = cmd_render(&req_path, &resp_path, "plain", opts);
        assert_eq!(code, EXIT_PROCESS_ERROR);
    }

    #[test]
    fn test_cmd_render_invalid_request_returns_validation_failed() {
        let dir = tempdir().unwrap();
        let req_path = dir.path().join("bad.json");
        let resp_path = dir.path().join("response.json");
        // Write a request with invalid interaction_id
        let bad_req = serde_json::json!({
            "schema": 1,
            "interaction_id": "bad-id",
            "kind": "approval_request",
            "status": "pending",
            "title": "Test",
            "message": "msg",
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
            color_depth: crate::interaction::terminal::ColorDepth::Mono,
            theme: "auto".to_string(),
            no_alternate_screen: false,
            timeout_seconds: 0,
        };
        let code = cmd_render(&req_path, &resp_path, "plain", opts);
        assert_eq!(code, EXIT_VALIDATION_FAILED);
    }

    #[test]
    fn test_cmd_probe_terminal_json() {
        let code = cmd_probe_terminal("json");
        assert_eq!(code, EXIT_SUCCESS);
    }

    #[test]
    fn test_cmd_probe_terminal_text() {
        let code = cmd_probe_terminal("text");
        assert_eq!(code, EXIT_SUCCESS);
    }

    #[test]
    fn test_read_request_ok() {
        let dir = tempdir().unwrap();
        let req_path = dir.path().join("req.json");
        write_sample_request(&req_path);
        let req = io::read_request(&req_path).unwrap();
        assert_eq!(req.interaction_id, "ix-20260318-001");
        assert_eq!(req.schema, 1);
    }

    #[test]
    fn test_selection_mode_to_str() {
        use crate::interaction::schema::SelectionMode;
        assert_eq!(selection_mode_to_str(&SelectionMode::SingleSelect), "single_select");
        assert_eq!(selection_mode_to_str(&SelectionMode::MultiSelect), "multi_select");
        assert_eq!(selection_mode_to_str(&SelectionMode::Boolean), "boolean");
    }
}
