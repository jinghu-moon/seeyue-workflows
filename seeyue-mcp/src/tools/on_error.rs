// src/tools/on_error.rs
//
// sy_on_error: Unified error handler hook.
// Records tool failures to journal, sends Toast notification,
// and returns structured recovery suggestions.

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::journal::{self, JournalEvent};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OnErrorParams {
    /// Tool name that failed (e.g. "edit", "run_command").
    pub tool:        String,
    /// Error message or structured error JSON.
    pub error:       String,
    /// Error kind hint: "io" | "syntax" | "lsp" | "policy" | "timeout" | "unknown".
    pub error_kind:  Option<String>,
    /// File path involved (if any).
    pub path:        Option<String>,
    /// Whether to send a Toast notification (default: false — errors are frequent).
    pub notify:      Option<bool>,
    /// Node id context.
    pub node_id:     Option<String>,
    /// Run id context.
    pub run_id:      Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OnErrorResult {
    #[serde(rename = "type")]
    pub kind:        String, // "recorded"
    pub tool:        String,
    pub error_kind:  String,
    pub recorded:    bool,
    pub notified:    bool,
    pub suggestions: Vec<String>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_on_error(
    params: OnErrorParams,
    workflow_dir: &Path,
) -> Result<OnErrorResult, ToolError> {
    if params.tool.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "tool".into(),
            hint:    "Provide the tool name that failed.".into(),
        });
    }

    let error_kind = params.error_kind.as_deref().unwrap_or("unknown").to_string();

    // Journal
    let evt = JournalEvent {
        event:   "tool_error".into(),
        actor:   "hook".into(),
        payload: Some(serde_json::json!({
            "tool":       params.tool,
            "error":      params.error,
            "error_kind": error_kind,
            "path":       params.path,
        })),
        phase:    None,
        node_id:  params.node_id.clone(),
        run_id:   params.run_id.clone(),
        ts:       Utc::now().to_rfc3339(),
        trace_id: None,
    };
    let recorded = journal::append_event(workflow_dir, evt).is_ok();

    // Optional Toast
    let notified = if params.notify.unwrap_or(false) {
        win_notify::send_toast(
            "seeyue-mcp [error]",
            &format!("{}: {}", params.tool, &params.error[..params.error.len().min(120)]),
            NotifyLevel::Warn,
        );
        true
    } else {
        false
    };

    let suggestions = build_suggestions(&error_kind, params.tool.as_str());

    Ok(OnErrorResult {
        kind:       "recorded".into(),
        tool:       params.tool,
        error_kind,
        recorded,
        notified,
        suggestions,
    })
}

// ─── Recovery suggestions ─────────────────────────────────────────────────────

fn build_suggestions(kind: &str, tool: &str) -> Vec<String> {
    let mut s: Vec<String> = Vec::new();
    match kind {
        "io" => {
            s.push("Check that the file path is relative to workspace root (forward slashes).".into());
            s.push("Use read_file first to verify the file exists.".into());
        }
        "syntax" => {
            s.push("Run verify_syntax to locate the syntax error.".into());
            s.push("Use preview_edit to review the proposed change before applying it.".into());
        }
        "lsp" => {
            s.push("LSP server may not be running. Check env var AGENT_EDITOR_LSP_CMD.".into());
            s.push("Fall back to search_workspace for symbol lookup.".into());
        }
        "policy" => {
            s.push("The operation was blocked by policy. Use sy_approval_request to request override.".into());
            s.push("Review workflow://session for active blockers.".into());
        }
        "timeout" => {
            s.push("Increase timeout_secs parameter if operation is expected to be slow.".into());
            s.push("Check for blocking processes with process_list.".into());
        }
        _ => {
            s.push(format!("Review the full error message from {} and retry.", tool));
            s.push("Consult workflow://journal for recent events context.".into());
        }
    }
    s
}
