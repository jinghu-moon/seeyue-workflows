// src/tools/approval.rs
//
// Approval tools:
//   - sy_approval_request: create a pending approval, send Windows toast, optional timeout
//   - sy_approval_resolve: mark approval approved/rejected
//   - sy_approval_status:  query pending or all approvals (supports since_ts filter)

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
    pub subject:      String,
    /// Optional longer description.
    pub detail:       Option<String>,
    /// Category tag (e.g. "destructive", "deploy", "policy").
    pub category:     Option<String>,
    /// Auto-reject after this many seconds if not resolved (omit = no timeout).
    pub timeout_secs: Option<u64>,
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
    /// Return only entries created at or after this ISO 8601 timestamp.
    pub since_ts:    Option<String>,
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
    pub timeout_secs: Option<u64>,
    pub expires_at:   Option<String>,
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
    pub approval_id:  String,
    pub subject:      String,
    pub category:     Option<String>,
    pub status:       String,
    pub ts:           String,
    pub detail:       Option<String>,
    pub expires_at:   Option<String>,
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
    approval_id:  String,
    ts:           String,
    subject:      String,
    detail:       Option<String>,
    category:     Option<String>,
    status:       String,   // "pending" | "approved" | "rejected" | "timeout"
    decision:     Option<String>,
    note:         Option<String>,
    resolved_at:  Option<String>,
    timeout_secs: Option<u64>,
    expires_at:   Option<String>,
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

/// Check if an approval has expired; if so, write a timeout record and return true.
fn expire_if_needed(workflow_dir: &Path, record: &ApprovalRecord) -> bool {
    if record.status != "pending" { return false; }
    let expires_at = match &record.expires_at {
        Some(e) => e.clone(),
        None    => return false,
    };
    let now = Utc::now().to_rfc3339();
    if now.as_str() < expires_at.as_str() { return false; }

    let timed_out = ApprovalRecord {
        status:      "timeout".into(),
        decision:    Some("rejected".into()),
        note:        Some("auto-rejected: timeout expired".into()),
        resolved_at: Some(now.clone()),
        ..ApprovalRecord {
            approval_id:  record.approval_id.clone(),
            ts:           record.ts.clone(),
            subject:      record.subject.clone(),
            detail:       record.detail.clone(),
            category:     record.category.clone(),
            timeout_secs: record.timeout_secs,
            expires_at:   record.expires_at.clone(),
            status:       "timeout".into(),
            decision:     Some("rejected".into()),
            note:         Some("auto-rejected: timeout expired".into()),
            resolved_at:  Some(now),
        }
    };
    let _ = append_record(workflow_dir, &timed_out);
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "approval_timeout".into(),
        actor:   "system".into(),
        payload: Some(serde_json::json!({ "approval_id": record.approval_id })),
        phase:    None,
        node_id:  None,
        run_id:   None,
        ts:       Utc::now().to_rfc3339(),
        trace_id: None,
    });
    true
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
    let ts          = Utc::now().to_rfc3339();

    // Compute expiry timestamp if timeout_secs provided
    let expires_at: Option<String> = params.timeout_secs.map(|secs| {
        (Utc::now() + chrono::Duration::seconds(secs as i64)).to_rfc3339()
    });

    let record = ApprovalRecord {
        approval_id:  approval_id.clone(),
        ts:           ts.clone(),
        subject:      params.subject.clone(),
        detail:       params.detail.clone(),
        category:     params.category.clone(),
        status:       "pending".into(),
        decision:     None,
        note:         None,
        resolved_at:  None,
        timeout_secs: params.timeout_secs,
        expires_at:   expires_at.clone(),
    };
    append_record(workflow_dir, &record)?;

    // Send Windows toast
    let timeout_hint = expires_at.as_deref()
        .map(|e| format!(" [expires {}]", &e[..19]))
        .unwrap_or_default();
    let toast_msg = params.detail.as_deref()
        .map(|d| format!("{} — {}{}", params.subject, d, timeout_hint))
        .unwrap_or_else(|| format!("{}{}", params.subject, timeout_hint));
    let toast = win_notify::send_toast("seeyue-mcp [approval]", &toast_msg, NotifyLevel::Warn);

    // Journal
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "approval_requested".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "approval_id":  approval_id,
            "subject":      params.subject,
            "category":     params.category,
            "timeout_secs": params.timeout_secs,
            "expires_at":   expires_at,
        })),
        phase:    None,
        node_id:  None,
        run_id:   None,
        ts:       Utc::now().to_rfc3339(),
        trace_id: None,
    });

    Ok(ApprovalRequestResult {
        kind:         "pending".into(),
        approval_id,
        subject:      record.subject,
        status:       "pending".into(),
        notified:     toast.notified,
        timeout_secs: params.timeout_secs,
        expires_at:   record.expires_at,
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
        approval_id:  existing.approval_id.clone(),
        ts:           existing.ts,
        subject:      existing.subject.clone(),
        detail:       existing.detail,
        category:     existing.category,
        timeout_secs: existing.timeout_secs,
        expires_at:   existing.expires_at,
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
        ts:       Utc::now().to_rfc3339(),
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
    let mut all = load_approvals(workflow_dir);

    // Expire any timed-out pending approvals before returning
    let ids: Vec<String> = all.iter().map(|r| r.approval_id.clone()).collect();
    for id in &ids {
        if let Some(r) = all.iter().find(|r| &r.approval_id == id) {
            let r_clone = ApprovalRecord {
                approval_id:  r.approval_id.clone(),
                ts:           r.ts.clone(),
                subject:      r.subject.clone(),
                detail:       r.detail.clone(),
                category:     r.category.clone(),
                status:       r.status.clone(),
                decision:     r.decision.clone(),
                note:         r.note.clone(),
                resolved_at:  r.resolved_at.clone(),
                timeout_secs: r.timeout_secs,
                expires_at:   r.expires_at.clone(),
            };
            expire_if_needed(workflow_dir, &r_clone);
        }
    }
    // Reload after potential expiry writes
    all = load_approvals(workflow_dir);

    let filtered: Vec<ApprovalEntry> = all.iter()
        .filter(|r| {
            // approval_id filter
            if let Some(ref id) = params.approval_id {
                if &r.approval_id != id { return false; }
            } else if r.status != "pending" {
                return false;
            }
            // since_ts filter
            if let Some(ref since) = params.since_ts {
                if r.ts.as_str() < since.as_str() { return false; }
            }
            true
        })
        .map(|r| ApprovalEntry {
            approval_id: r.approval_id.clone(),
            subject:     r.subject.clone(),
            category:    r.category.clone(),
            status:      r.status.clone(),
            ts:          r.ts.clone(),
            detail:      r.detail.clone(),
            expires_at:  r.expires_at.clone(),
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

// ─── P2-N3: Interaction Projection ───────────────────────────────────────────
//
// Projects a legacy approval request into the interaction store layout
// (.ai/workflow/interactions/requests/). This is additive-only — the existing
// approval tools and approvals.jsonl are NOT modified.

/// Result of projecting an approval into the interaction store.
#[derive(Debug, Serialize)]
pub struct InteractionProjectionResult {
    pub interaction_id: String,
    pub kind:           String,  // "approval_request"
    pub schema:         u32,     // 1
    pub file_path:      String,
    pub projected:      bool,
}

/// Write an interaction request file for an approval.
/// Path: <workflow_dir>/interactions/requests/<approval_id>.json
/// The file follows the interaction.schema.yaml v1 layout.
pub fn project_approval_as_interaction(
    approval_id: &str,
    subject: &str,
    detail: Option<&str>,
    category: Option<&str>,
    workflow_dir: &Path,
) -> Result<InteractionProjectionResult, ToolError> {
    let requests_dir = workflow_dir.join("interactions").join("requests");
    fs::create_dir_all(&requests_dir)
        .map_err(|e| ToolError::IoError { message: format!("create interactions/requests dir: {e}") })?;

    let ts = Utc::now().to_rfc3339();
    let interaction_id = format!("ix-{}-000", &ts[..10].replace('-', ""));

    let obj = serde_json::json!({
        "schema": 1,
        "interaction_id": interaction_id,
        "kind": "approval_request",
        "status": "pending",
        "title": subject,
        "message": detail.unwrap_or(subject),
        "selection_mode": "boolean",
        "options": [
            {"id": "approve", "label": "Approve", "recommended": true},
            {"id": "reject",  "label": "Reject",  "recommended": false}
        ],
        "comment_mode": "disabled",
        "presentation": {
            "mode": "text_menu",
            "color_profile": "auto",
            "theme": "auto"
        },
        "originating_request_id": approval_id,
        "risk_level": category.unwrap_or("medium"),
        "created_at": ts,
    });

    let file_path = requests_dir.join(format!("{}.json", approval_id));
    let content = serde_json::to_string_pretty(&obj)
        .map_err(|e| ToolError::IoError { message: format!("serialize interaction: {e}") })?;
    fs::write(&file_path, format!("{content}\n"))
        .map_err(|e| ToolError::IoError { message: format!("write interaction file: {e}") })?;

    Ok(InteractionProjectionResult {
        interaction_id,
        kind:      "approval_request".into(),
        schema:    1,
        file_path: file_path.to_string_lossy().into_owned(),
        projected: true,
    })
}
