// src/tools/hooks.rs
//
// Six MCP hook tools that call the policy engine directly (zero IPC):
//   - sy_pretool_bash:     command classification + approval check
//   - sy_pretool_write:    file classification + TDD + scope drift
//   - sy_posttool_write:   record write evidence to journal
//   - sy_stop:             check stop conditions
//   - sy_create_checkpoint: create checkpoint + journal event
//   - sy_advance_node:     update session.yaml node + journal event

use rmcp::schemars;
use serde::Deserialize;

use crate::AppState;
use crate::policy::types::HookResult;
use crate::workflow::{journal, state};
use crate::tools::compact_journal::{CompactJournalParams, run_compact_journal};

const AUTO_FLUSH_THRESHOLD: usize = 150;
const AUTO_FLUSH_RETAIN:    usize = 100;

// ─── Tool Parameters ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreToolBashParams {
    #[schemars(description = "The shell command about to be executed")]
    pub command: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreToolWriteParams {
    #[schemars(description = "File path (relative to workspace) about to be written")]
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PostToolWriteParams {
    #[schemars(description = "File path that was written")]
    pub path: String,
    #[serde(default)]
    #[schemars(description = "Tool that performed the write (write/edit/multi_edit)")]
    pub tool: Option<String>,
    #[serde(default)]
    #[schemars(description = "Number of lines changed")]
    pub lines_changed: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StopParams {
    #[serde(default)]
    #[schemars(description = "Reason for stopping (optional context)")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateCheckpointParams {
    #[schemars(description = "Human-readable label for this checkpoint")]
    pub label: String,
    #[serde(default)]
    #[schemars(description = "Files to snapshot (empty = no files, just journal event)")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AdvanceNodeParams {
    #[schemars(description = "New node ID to advance to")]
    pub node_id: String,
    #[serde(default)]
    #[schemars(description = "New node name (optional)")]
    pub name: Option<String>,
    #[serde(default)]
    #[schemars(description = "Initial node status (default: active)")]
    pub status: Option<String>,
    #[serde(default)]
    #[schemars(description = "Initial node state (e.g., red_pending for TDD)")]
    pub state: Option<String>,
    #[serde(default)]
    #[schemars(description = "Whether TDD is required for this node")]
    pub tdd_required: Option<bool>,
    #[serde(default)]
    #[schemars(description = "Target file paths for scope tracking")]
    pub target: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SessionStartParams {
    #[serde(default)]
    #[schemars(description = "Skip crash-recovery journal replay (default: false)")]
    pub skip_recovery: Option<bool>,
}

// ─── Tool Implementations ────────────────────────────────────────────────────

/// Execute sy_pretool_bash: classify command and check policy.
pub fn run_pretool_bash(params: PreToolBashParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);
    app.policy_engine.check_bash(&params.command, &session)
}

/// Execute sy_pretool_write: classify file and check policy.
/// If the policy allows and the target file already exists, auto-capture a
/// pre-destructive checkpoint so the write can be rewound via `rewind`.
pub fn run_pretool_write(params: PreToolWriteParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);
    let result = app.policy_engine.check_write(&params.path, &session);

    // Pre-destructive checkpoint: snapshot existing file before an overwrite.
    if result.verdict == crate::policy::types::Verdict::Allow {
        let full_path = app.workspace.join(&params.path);
        if full_path.exists() {
            let call_id = format!(
                "pre_write_{}",
                chrono::Utc::now().timestamp_millis()
            );
            // Non-fatal: checkpoint failure must not block the write.
            let _ = app.checkpoint.capture(&full_path, &call_id, "sy_pretool_write");
        }
    }

    result
}

/// Execute sy_posttool_write: record write evidence to journal.
pub fn run_posttool_write(params: PostToolWriteParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);
    let run_id  = session.run_id.as_deref().unwrap_or("").to_string();
    let phase   = session.phase.id.as_deref().or(session.phase.name.as_deref()).unwrap_or("none").to_string();
    let node_id = session.node.id.as_deref().or(session.node.name.as_deref()).unwrap_or("none").to_string();
    let checkpoint_label = session.recovery.last_checkpoint_id.clone();

    // Compute before hash (from .sy-bak) and after hash (current file)
    let full_path = app.workspace.join(&params.path);
    let before_hash = {
        let bak = full_path.with_extension(
            format!("{}.sy-bak", full_path.extension().and_then(|e| e.to_str()).unwrap_or(""))
        );
        if bak.exists() {
            std::fs::read(&bak).ok().map(|b| hex_sha256(&b))
        } else {
            None
        }
    };
    let after_hash = if full_path.exists() {
        std::fs::read(&full_path).ok().map(|b| hex_sha256(&b))
    } else {
        None
    };

    if let Err(e) = journal::record_write_evidence(journal::WriteEvidenceParams {
        workflow_dir:     &app.workflow_dir,
        run_id:           &run_id,
        phase:            &phase,
        node_id:          &node_id,
        tool:             params.tool.as_deref().unwrap_or("unknown"),
        path:             &params.path,
        lines_changed:    params.lines_changed.map(|v| v as i64),
        outcome:          "success",
        checkpoint_label: checkpoint_label.as_deref(),
        syntax_valid:     None,
        scope_drift:      false,
        before_hash,
        after_hash,
    }) {
        return HookResult::allow(format!("Write recorded (journal warning: {})", e));
    }

    // Auto-flush: compact journal if it exceeds threshold
    let line_count = journal::count_lines(&app.workflow_dir);
    let auto_compacted = if line_count > AUTO_FLUSH_THRESHOLD {
        run_compact_journal(
            CompactJournalParams { max_entries: Some(AUTO_FLUSH_RETAIN), summarize: false },
            &app.workflow_dir,
        ).is_ok()
    } else {
        false
    };

    if auto_compacted {
        HookResult::allow(format!("Write recorded: {} (auto_compacted: journal flushed)", params.path))
    } else {
        HookResult::allow(format!("Write recorded: {}", params.path))
    }
}

/// Execute sy_stop: check whether the session can stop.
pub fn run_stop(params: StopParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);
    let result = app.policy_engine.check_stop(&session);

    // Record stop attempt in journal
    let event = journal::JournalEvent::new("stop_attempted", "hook")
        .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
        .with_payload(serde_json::json!({
            "verdict": result.verdict.to_string(),
            "reason": params.reason,
        }));

    let _ = journal::append_event(&app.workflow_dir, event);

    result
}

/// Execute sy_create_checkpoint: snapshot files + journal event.
pub fn run_create_checkpoint(params: CreateCheckpointParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);

    // Snapshot files if any
    if let Some(files) = &params.files {
        for file_path in files {
            let full_path = app.workspace.join(file_path);
            if full_path.exists() {
                let call_id = format!("checkpoint_{}", chrono::Utc::now().timestamp_millis());
                if let Err(e) = app.checkpoint.capture(&full_path, &call_id, "checkpoint") {
                    return HookResult::block(format!(
                        "Checkpoint failed for {}: {:?}",
                        file_path, e
                    ));
                }
            }
        }
    }

    // Journal event
    let event = journal::JournalEvent::new("checkpoint_created", "hook")
        .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
        .with_phase(session.phase.id.as_deref().unwrap_or("unknown"))
        .with_node_id(session.node.id.as_deref().unwrap_or("unknown"))
        .with_payload(serde_json::json!({
            "label": params.label,
            "files": params.files,
        }));

    if let Err(e) = journal::append_event(&app.workflow_dir, event) {
        return HookResult::allow(format!(
            "Checkpoint created: {} (journal warning: {})",
            params.label, e
        ));
    }

    HookResult::allow(format!("Checkpoint created: {}", params.label))
}

/// Execute sy_advance_node: update session.yaml and record journal event.
pub fn run_advance_node(params: AdvanceNodeParams, app: &AppState) -> HookResult {
    let mut session = state::load_session(&app.workflow_dir);

    // Loop budget guard — block before advancing if any metric is exceeded
    if let Some(reason) = state::check_loop_budget(&session) {
        return HookResult::block(format!("sy_advance_node blocked: {}", reason));
    }

    let old_node_id = session.node.id.clone();

    // Update node fields
    session.node.id = Some(params.node_id.clone());
    session.node.name = params.name.clone();
    session.node.status = Some(params.status.unwrap_or_else(|| "active".to_string()));
    session.node.state = params.state.clone();
    session.node.tdd_required = params.tdd_required;
    session.node.target = params.target.clone();
    session.node.tdd_state = if params.tdd_required == Some(true) {
        Some("red_pending".to_string())
    } else {
        None
    };

    // Save session
    if let Err(e) = state::save_session(&app.workflow_dir, &session) {
        return HookResult::block(format!("Failed to save session: {}", e));
    }

    // Journal events
    if let Some(old_id) = &old_node_id {
        let exit_event = journal::JournalEvent::new("node_exited", "hook")
            .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
            .with_node_id(old_id)
            .with_payload(serde_json::json!({
                "next_node": params.node_id,
            }));
        let _ = journal::append_event(&app.workflow_dir, exit_event);
    }

    let enter_event = journal::JournalEvent::new("node_entered", "hook")
        .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
        .with_phase(session.phase.id.as_deref().unwrap_or("unknown"))
        .with_node_id(&params.node_id)
        .with_payload(serde_json::json!({
            "name": params.name,
            "tdd_required": params.tdd_required,
            "target": params.target,
        }));
    let _ = journal::append_event(&app.workflow_dir, enter_event);

    HookResult::allow(format!("Advanced to node: {}", params.node_id))
}

/// Execute sy_session_start: bootstrap session + crash-recovery journal replay.
///
/// Recovery protocol (architecture-v4.md §8 P3-A):
/// 1. Load session.yaml
/// 2. Scan journal.jsonl for orphan tool_request events (request with no completion)
/// 3. Append `aborted` event for each orphan
/// 4. Determine safe resume point from TDD state machine
/// 5. Return structured session summary + recovery status
pub fn run_session_start(params: SessionStartParams, app: &AppState) -> serde_json::Value {
    let session = state::load_session(&app.workflow_dir);
    let skip = params.skip_recovery.unwrap_or(false);

    let mut orphans_found = 0u32;
    let mut recovery_status = "clean";
    let mut resume_tdd_state: Option<String> = None;

    if !skip {
        // Scan journal for orphan events
        let journal_path = app.workflow_dir.join("journal.jsonl");
        if let Ok(content) = std::fs::read_to_string(&journal_path) {
            let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

            // Track tool_request events that have no matching completion
            let mut pending_requests: Vec<String> = Vec::new();
            for line in &lines {
                if let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) {
                    let event = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    let trace = ev.get("trace_id").and_then(|v| v.as_str()).unwrap_or("");
                    match event {
                        "tool_request" => {
                            if !trace.is_empty() {
                                pending_requests.push(trace.to_string());
                            }
                        }
                        "tool_completed" | "tool_failed" | "aborted" => {
                            pending_requests.retain(|t| t != trace);
                        }
                        _ => {}
                    }
                }
            }

            // Write aborted events for all remaining orphans
            for trace_id in &pending_requests {
                orphans_found += 1;
                let ev = journal::JournalEvent::new("aborted", "session_start")
                    .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
                    .with_payload(serde_json::json!({
                        "reason": "orphan_on_session_start",
                        "original_trace_id": trace_id,
                    }));
                let _ = journal::append_event(&app.workflow_dir, ev);
            }

            if orphans_found > 0 {
                recovery_status = "recovered";
            }
        }

        // Determine safe TDD resume point from current node state
        let tdd_state = session.node.tdd_state.as_deref().unwrap_or("");
        resume_tdd_state = Some(match tdd_state {
            // Already have RED evidence — can proceed to GREEN
            "red_verified" => "red_verified".to_string(),
            // Already have GREEN evidence — can proceed to REFACTOR
            "green_verified" => "green_verified".to_string(),
            // In the middle of a step — restart that step
            "red_pending" | "green_pending" | "refactor_pending" =>
                tdd_state.to_string(),
            // Unknown / no TDD — safe default
            _ => "idle".to_string(),
        });
    }

    // Record session_start event
    let ev = journal::JournalEvent::new("session_started", "session_start")
        .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
        .with_phase(session.phase.id.as_deref().unwrap_or("unknown"))
        .with_node_id(session.node.id.as_deref().unwrap_or("unknown"))
        .with_payload(serde_json::json!({
            "orphans_recovered": orphans_found,
            "recovery_status": recovery_status,
            "skip_recovery": skip,
        }));
    let _ = journal::append_event(&app.workflow_dir, ev);

    // Check loop budget
    let budget_status = state::check_loop_budget(&session);

    // Load boot_memory: top 5 recently-updated memory entries from .ai/memory/index.json
    let boot_memory = load_boot_memory(&app.workspace);

    serde_json::json!({
        "run_id":             session.run_id,
        "phase":              session.phase.id,
        "node_id":            session.node.id,
        "node_name":          session.node.name,
        "tdd_state":          session.node.tdd_state,
        "resume_tdd_state":   resume_tdd_state,
        "recovery_status":    recovery_status,
        "orphans_recovered":  orphans_found,
        "budget_exceeded":    budget_status,
        "recovery_required":  session.recovery.status.as_deref() == Some("restore_pending"),
        "restore_reason":     session.recovery.restore_reason,
        "boot_memory":        boot_memory,
    })
}

/// Load top 5 recently-updated memory hints from .ai/memory/index.json.
/// Returns empty array if no memory store exists yet.
fn load_boot_memory(workspace: &std::path::Path) -> serde_json::Value {
    let index_path = workspace.join(".ai/memory/index.json");
    if !index_path.exists() {
        return serde_json::json!([]);
    }
    let raw = match std::fs::read_to_string(&index_path) {
        Ok(s) => s,
        Err(_) => return serde_json::json!([]),
    };
    let index: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&raw).unwrap_or_default();

    let mut entries: Vec<(String, serde_json::Value)> = index.into_iter().collect();
    // Sort by updated descending
    entries.sort_by(|a, b| {
        let ta = a.1.get("updated").and_then(|v| v.as_str()).unwrap_or("");
        let tb = b.1.get("updated").and_then(|v| v.as_str()).unwrap_or("");
        tb.cmp(ta)
    });
    entries.truncate(5);

    serde_json::json!(entries.into_iter().map(|(key, entry)| serde_json::json!({
        "key":     key,
        "tags":    entry.get("tags"),
        "updated": entry.get("updated"),
        "preview": entry.get("preview"),
    })).collect::<Vec<_>>())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Compute SHA-256 hex digest of a byte slice.
fn hex_sha256(data: &[u8]) -> String {
    crate::workflow::journal::hex_sha256(data)
}
