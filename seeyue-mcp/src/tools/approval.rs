// src/tools/approval.rs
//
// Three approval tools:
//   - sy_approval_request: create a pending approval, send Windows toast
//   - sy_approval_resolve: mark approval approved/rejected
//   - sy_approval_status:  query pending or all approvals

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::journal::{self, JournalEvent};

const APPROVALS_FILE: &str = "approvals.jsonl";

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApprovalRequestParams {
    /// Short subject line shown in the toast and approval list.
    pub subject:     String,
    /// Optional longer description.
    pub detail:      Option<String>,
    /// Category tag (e.g. "destructive", "deploy", "policy").
    pub category:    Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalResolveParams {
    /// Approval ID returned by sy_approval_request.
    pub approval_id: String,
    /// Decision: "approved" | "rejected"
    pub decision:    String,
    /// Optional note recorded with the resolution.
    pub note:        Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalStatusParams {
    /// If provided, fetch a specific approval. Otherwise returns all pending.
    pub approval_id: Option<String>,
}

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApprovalRequestResult {
    #[serde(rename = "type")]
    pub kind:        String, // "pending"
    pub approval_id: String,
    pub subject:     String,
    pub status:      String,
    pub notified:    bool,
}

#[derive(Debug, Serialize)]
pub struct ApprovalResolveResult {
    #[serde(rename = "type")]
    pub kind:        String,
    pub approval_id: String,
    pub decision:    String,
    pub resolved_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApprovalEntry {
    pub approval_id: String,
    pub subject:     String,
    pub category:    Option<String>,
    pub status:      String,
    pub ts:          String,
    pub detail:      Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApprovalStatusResult {
    #[serde(rename = "type")]
    pub kind:          String,
    pub total:         usize,
    pub pending_count: usize,
    pub approvals:     Vec<ApprovalEntry>,
}

// ─── Internal record ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct ApprovalRecord {
    approval_id: String,
    ts:          String,
    subject:     String,
    detail:      Option<String>,
    category:    Option<String>,
    status:      String,   // "pending" | "approved" | "rejected"
    decision:    Option<String>,
    note:        Option<String>,
    resolved_at: Option<String>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn approvals_path(workflow_dir: &Path) -> std::path::PathBuf {
    workflow_dir.join(APPROVALS_FILE)
}

fn load_approvals(workflow_dir: &Path) -> Vec<ApprovalRecord> {
    let path = approvals_path(workflow_dir);
    if !path.exists() { return vec![]; }
    let content = fs::read_to_string(&path).unwrap_or_default();
    // Last record per approval_id wins
    let mut map: std::collections::HashMap<String, ApprovalRecord> =
        std::collections::HashMap::new();
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        if let Ok(r) = serde_json::from_str::<ApprovalRecord>(line) {
            map.insert(r.approval_id.clone(), r);
        }
    }
    let mut records: Vec<ApprovalRecord> = map.into_values().collect();
    records.sort_by(|a, b| b.ts.cmp(&a.ts));
    records
}

fn append_record(workflow_dir: &Path, record: &ApprovalRecord) -> Result<(), ToolError> {
    let path = approvals_path(workflow_dir);
    let line = serde_json::to_string(record)
        .map_err(|e| ToolError::IoError { message: format!("serialize approval: {e}") })?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| ToolError::IoError { message: format!("open approvals: {e}") })?;
    writeln!(file, "{}", line)
        .map_err(|e| ToolError::IoError { message: format!("write approval: {e}") })?;
    Ok(())
}

// ─── sy_approval_request ─────────────────────────────────────────────────────

pub fn run_approval_request(
    params: ApprovalRequestParams,
    workflow_dir: &Path,
) -> Result<ApprovalRequestResult, ToolError> {
    if params.subject.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "subject".into(),
            hint:    "Provide a non-empty approval subject.".into(),
        });
    }

    let approval_id = format!("apr_{}", Utc::now().timestamp_millis());
    let ts = Utc::now().to_rfc3339();

    let record = ApprovalRecord {
        approval_id: approval_id.clone(),
        ts:          ts.clone(),
        subject:     params.subject.clone(),
        detail:      params.detail.clone(),
        category:    params.category.clone(),
        status:      "pending".into(),
        decision:    None,
        note:        None,
        resolved_at: None,
    };
    append_record(workflow_dir, &record)?;

    // Send Windows toast
    let toast_msg = params.detail.as_deref()
        .map(|d| format!("{} — {}", params.subject, d))
        .unwrap_or_else(|| params.subject.clone());
    let toast = win_notify::send_toast("seeyue-mcp [approval]", &toast_msg, NotifyLevel::Warn);

    // Journal
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "approval_requested".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "approval_id": approval_id,
            "subject":     params.subject,
            "category":    params.category,
        })),
        phase:    None,
        node_id:  None,
        run_id:   None,
        ts:       chrono::Utc::now().to_rfc3339(),
        trace_id: None,
    });

    Ok(ApprovalRequestResult {
        kind:        "pending".into(),
        approval_id,
        subject:     record.subject,
        status:      "pending".into(),
        notified:    toast.notified,
    })
}

// ─── sy_approval_resolve ─────────────────────────────────────────────────────

pub fn run_approval_resolve(
    params: ApprovalResolveParams,
    workflow_dir: &Path,
) -> Result<ApprovalResolveResult, ToolError> {
    let valid = ["approved", "rejected"];
    if !valid.contains(&params.decision.as_str()) {
        return Err(ToolError::IoError {
            message: format!("decision must be 'approved' or 'rejected', got '{}'.", params.decision),
        });
    }

    let all = load_approvals(workflow_dir);
    if !all.iter().any(|r| r.approval_id == params.approval_id) {
        return Err(ToolError::IoError {
            message: format!("approval_id '{}' not found.", params.approval_id),
        });
    }

    let resolved_at = Utc::now().to_rfc3339();
    let existing = all.into_iter()
        .find(|r| r.approval_id == params.approval_id)
        .unwrap();

    let updated = ApprovalRecord {
        status:      params.decision.clone(),
        decision:    Some(params.decision.clone()),
        note:        params.note.clone(),
        resolved_at: Some(resolved_at.clone()),
        ..existing
    };
    append_record(workflow_dir, &updated)?;

    // Notify resolution
    let _ = win_notify::send_toast(
        "seeyue-mcp [approval]",
        &format!("{}: {}", updated.subject, params.decision.to_uppercase()),
        NotifyLevel::Info,
    );

    // Journal
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "approval_resolved".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "approval_id": params.approval_id,
            "decision":    params.decision,
            "note":        params.note,
        })),
        phase:    None,
        node_id:  None,
        run_id:   None,
        ts:       chrono::Utc::now().to_rfc3339(),
        trace_id: None,
    });

    Ok(ApprovalResolveResult {
        kind:        params.decision.clone(),
        approval_id: params.approval_id,
        decision:    params.decision,
        resolved_at,
    })
}

// ─── sy_approval_status ──────────────────────────────────────────────────────

pub fn run_approval_status(
    params: ApprovalStatusParams,
    workflow_dir: &Path,
) -> Result<ApprovalStatusResult, ToolError> {
    let all = load_approvals(workflow_dir);

    let filtered: Vec<ApprovalEntry> = all.iter()
        .filter(|r| {
            if let Some(ref id) = params.approval_id {
                &r.approval_id == id
            } else {
                r.status == "pending"
            }
        })
        .map(|r| ApprovalEntry {
            approval_id: r.approval_id.clone(),
            subject:     r.subject.clone(),
            category:    r.category.clone(),
            status:      r.status.clone(),
            ts:          r.ts.clone(),
            detail:      r.detail.clone(),
        })
        .collect();

    let pending_count = filtered.iter().filter(|e| e.status == "pending").count();
    let kind = if filtered.is_empty() { "empty" } else { "success" };

    Ok(ApprovalStatusResult {
        kind:          kind.into(),
        total:         filtered.len(),
        pending_count,
        approvals:     filtered,
    })
}
