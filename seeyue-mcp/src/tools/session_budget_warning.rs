// src/tools/session_budget_warning.rs
//
// sy_session_budget_warning: Check current loop budget usage and send a
// Windows Toast warning when approaching the limit (default: 80% threshold).
// Designed to be called from PreToolUse:Bash hook context or by the agent.

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::journal::{self, JournalEvent};
use crate::workflow::state;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct SessionBudgetWarningParams {
    /// Warning threshold as a fraction 0.0–1.0 (default: 0.8 = 80%).
    pub threshold:    Option<f32>,
    /// Send a Toast notification if threshold is exceeded (default: true).
    pub notify:       Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SessionBudgetWarningResult {
    #[serde(rename = "type")]
    pub kind:            String, // "ok" | "warning" | "exceeded" | "no_budget"
    pub consumed:        Option<u32>,
    pub limit:           Option<u32>,
    pub usage_pct:       Option<f32>,
    pub threshold_pct:   f32,
    pub budget_exceeded: bool,
    pub notified:        bool,
    pub message:         String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_session_budget_warning(
    params: SessionBudgetWarningParams,
    workflow_dir: &Path,
) -> Result<SessionBudgetWarningResult, ToolError> {
    let threshold = params.threshold.unwrap_or(0.8).clamp(0.0, 1.0);
    let do_notify = params.notify.unwrap_or(true);

    let session = state::load_session(workflow_dir);

    let budget = &session.loop_budget;

    // Resolve consumed / limit from V4 or legacy fields
    let (consumed, limit) = if let (Some(c), Some(m)) = (budget.consumed_nodes, budget.max_nodes) {
        (Some(c), Some(m))
    } else if let (Some(u), Some(m)) = (budget.used, budget.max) {
        (Some(u), Some(m))
    } else {
        (None, None)
    };

    let (kind, usage_pct, budget_exceeded, message) = match (consumed, limit) {
        (Some(c), Some(m)) if m > 0 => {
            let pct = c as f32 / m as f32;
            let exceeded = state::check_loop_budget(&session).is_some();
            let kind = if exceeded {
                "exceeded"
            } else if pct >= threshold {
                "warning"
            } else {
                "ok"
            };
            let msg = format!(
                "Loop budget: {}/{} nodes consumed ({:.0}%){}",
                c, m, pct * 100.0,
                if exceeded { " — EXCEEDED" } else if pct >= threshold { " — WARNING" } else { "" }
            );
            (kind.to_string(), Some(pct), exceeded, msg)
        }
        _ => (
            "no_budget".into(),
            None,
            false,
            "No loop budget configured in session.".into(),
        ),
    };

    let mut notified = false;
    if do_notify && (kind == "warning" || kind == "exceeded") {
        let level = if kind == "exceeded" { NotifyLevel::Warn } else { NotifyLevel::Info };
        let toast = win_notify::send_toast("seeyue-mcp [budget]", &message, level);
        notified = toast.notified;

        // Journal
        let _ = journal::append_event(workflow_dir, JournalEvent {
            event:   "budget_warning".into(),
            actor:   "tool".into(),
            payload: Some(serde_json::json!({
                "consumed":  consumed,
                "limit":     limit,
                "usage_pct": usage_pct,
                "kind":      kind,
            })),
            phase:    session.phase.id.or(session.phase.name),
            node_id:  session.node.id.or(session.node.name),
            run_id:   session.run_id,
            ts:       Utc::now().to_rfc3339(),
            trace_id: None,
        });
    }

    Ok(SessionBudgetWarningResult {
        kind,
        consumed,
        limit,
        usage_pct,
        threshold_pct: threshold,
        budget_exceeded,
        notified,
        message,
    })
}
