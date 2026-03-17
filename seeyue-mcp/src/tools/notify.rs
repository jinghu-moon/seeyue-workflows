// src/tools/notify.rs
//
// sy_notify: Send a Windows Toast notification and record to journal.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::journal::{self, JournalEvent};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SyNotifyParams {
    /// Notification body message.
    pub message: String,
    /// Level: info (default) | warn | milestone
    pub level:   Option<String>,
    /// Optional title override (default: "seeyue-mcp").
    pub title:   Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyNotifyResult {
    #[serde(rename = "type")]
    pub kind:     String, // "sent" | "fallback" | "error"
    pub message:  String,
    pub level:    String,
    pub notified: bool,
    pub method:   String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_sy_notify(
    params: SyNotifyParams,
    workflow_dir: &Path,
) -> Result<SyNotifyResult, ToolError> {
    if params.message.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "message".into(),
            hint:    "Provide a non-empty notification message.".into(),
        });
    }

    let level     = NotifyLevel::from_str(params.level.as_deref().unwrap_or("info"));
    let title     = params.title.as_deref().unwrap_or("seeyue-mcp").to_string();
    let level_str = level.as_str().to_string();

    let toast = win_notify::send_toast(&title, &params.message, level);

    // Record to journal
    let _ = journal::append_event(
        workflow_dir,
        JournalEvent {
            event:   "notification_sent".into(),
            actor:   "tool".into(),
            payload: Some(serde_json::json!({
                "message": params.message,
                "level":   level_str,
                "title":   title,
                "method":  toast.method,
                "notified": toast.notified,
            })),
            phase:    None,
            node_id:  None,
            run_id:   None,
            ts:       chrono::Utc::now().to_rfc3339(),
            trace_id: None,
        },
    );

    let kind = if !toast.notified {
        "error"
    } else if toast.method == "fallback" {
        "fallback"
    } else {
        "sent"
    };

    Ok(SyNotifyResult {
        kind:     kind.into(),
        message:  params.message,
        level:    level_str,
        notified: toast.notified,
        method:   toast.method.into(),
    })
}
