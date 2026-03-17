// src/tools/notify.rs
//
// sy_notify: Send a Windows Toast notification and record to journal.
// Supports basic toast and progress-bar toast.

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel, ToastProgress};
use crate::workflow::journal::{self, JournalEvent};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct NotifyProgressParams {
    /// Progress value 0.0–1.0; negative = indeterminate.
    pub value:  f32,
    /// Denominator label (e.g. "100").
    pub max:    Option<String>,
    /// Label above bar (e.g. "Building…").
    pub label:  Option<String>,
    /// Status text below bar (e.g. "42 / 100 nodes").
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyNotifyParams {
    /// Notification body message.
    pub message:  String,
    /// Level: info (default) | warn | milestone
    pub level:    Option<String>,
    /// Optional title override (default: "seeyue-mcp").
    pub title:    Option<String>,
    /// Optional progress bar.
    pub progress: Option<NotifyProgressParams>,
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

    let toast = match params.progress {
        Some(p) => win_notify::send_toast_progress(
            &title,
            &params.message,
            level,
            ToastProgress {
                value:  p.value,
                max:    p.max,
                label:  p.label,
                status: p.status,
            },
        ),
        None => win_notify::send_toast(&title, &params.message, level),
    };

    // Record to journal
    let _ = journal::append_event(
        workflow_dir,
        JournalEvent {
            event:   "notification_sent".into(),
            actor:   "tool".into(),
            payload: Some(serde_json::json!({
                "message":  params.message,
                "level":    level_str,
                "title":    title,
                "method":   toast.method,
                "notified": toast.notified,
            })),
            phase:    None,
            node_id:  None,
            run_id:   None,
            ts:       Utc::now().to_rfc3339(),
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
