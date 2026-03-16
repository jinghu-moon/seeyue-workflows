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

// ─── Tool Implementations ────────────────────────────────────────────────────

/// Execute sy_pretool_bash: classify command and check policy.
pub fn run_pretool_bash(params: PreToolBashParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);
    app.policy_engine.check_bash(&params.command, &session)
}

/// Execute sy_pretool_write: classify file and check policy.
pub fn run_pretool_write(params: PreToolWriteParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);
    app.policy_engine.check_write(&params.path, &session)
}

/// Execute sy_posttool_write: record write evidence to journal.
pub fn run_posttool_write(params: PostToolWriteParams, app: &AppState) -> HookResult {
    let session = state::load_session(&app.workflow_dir);

    let event = journal::JournalEvent::new("write_recorded", "hook")
        .with_run_id(session.run_id.as_deref().unwrap_or("unknown"))
        .with_phase(session.phase.id.as_deref().unwrap_or("unknown"))
        .with_node_id(session.node.id.as_deref().unwrap_or("unknown"))
        .with_payload(serde_json::json!({
            "path": params.path,
            "tool": params.tool.unwrap_or_else(|| "unknown".to_string()),
            "lines_changed": params.lines_changed,
        }));

    if let Err(e) = journal::append_event(&app.workflow_dir, event) {
        return HookResult::allow(format!("Write recorded (journal warning: {})", e));
    }

    HookResult::allow(format!("Write recorded: {}", params.path))
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
