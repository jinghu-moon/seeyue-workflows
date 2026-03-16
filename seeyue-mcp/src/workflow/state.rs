// src/workflow/state.rs
//
// Reads and manages `.ai/workflow/session.yaml` — the single source of truth
// for workflow state including phase, node, loop budget, approvals, and recovery.

use std::path::{Path, PathBuf};
use std::fs;

use serde::{Deserialize, Serialize};

// ─── Session State ───────────────────────────────────────────────────────────

/// Top-level session state from `.ai/workflow/session.yaml`.
/// Uses `serde_yaml` with `default` to gracefully handle missing fields.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub phase: PhaseState,
    #[serde(default)]
    pub node: NodeState,
    #[serde(default)]
    pub loop_budget: LoopBudget,
    #[serde(default)]
    pub approvals: ApprovalState,
    #[serde(default)]
    pub recovery: RecoveryState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhaseState {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeState {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub target: Option<Vec<String>>,
    #[serde(default)]
    pub tdd_required: Option<bool>,
    #[serde(default)]
    pub tdd_state: Option<String>,
    #[serde(default)]
    pub test_contract: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoopBudget {
    #[serde(default)]
    pub max: Option<u32>,
    #[serde(default)]
    pub used: Option<u32>,
    #[serde(default)]
    pub exhausted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalState {
    #[serde(default)]
    pub pending: Option<Vec<serde_yaml::Value>>,
    #[serde(default)]
    pub grants: Option<Vec<serde_yaml::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecoveryState {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub restore_reason: Option<String>,
}

// ─── Load / Save ─────────────────────────────────────────────────────────────

/// Load session state from `.ai/workflow/session.yaml`.
/// Returns `Default` if the file doesn't exist.
pub fn load_session(workflow_dir: &Path) -> SessionState {
    let path = workflow_dir.join("session.yaml");
    match fs::read_to_string(&path) {
        Ok(content) => {
            serde_yaml::from_str(&content).unwrap_or_default()
        }
        Err(_) => SessionState::default(),
    }
}

/// Save session state back to `.ai/workflow/session.yaml`.
pub fn save_session(workflow_dir: &Path, state: &SessionState) -> Result<(), String> {
    let path = workflow_dir.join("session.yaml");

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create workflow dir: {}", e))?;
    }

    let content = serde_yaml::to_string(state)
        .map_err(|e| format!("Failed to serialize session state: {}", e))?;

    fs::write(&path, content)
        .map_err(|e| format!("Failed to write session.yaml: {}", e))?;

    Ok(())
}

// ─── Query helpers ───────────────────────────────────────────────────────────

/// Check loop budget. Returns `Some(reason)` if budget is exhausted.
pub fn check_loop_budget(session: &SessionState) -> Option<String> {
    if session.loop_budget.exhausted == Some(true) {
        return Some("Loop budget exhausted".to_string());
    }

    if let (Some(max), Some(used)) = (session.loop_budget.max, session.loop_budget.used) {
        if used >= max {
            return Some(format!(
                "Loop budget exhausted: {}/{} iterations used",
                used, max
            ));
        }
    }

    None
}

/// Check if there are pending approvals.
pub fn has_pending_approvals(session: &SessionState) -> bool {
    session.approvals.pending
        .as_ref()
        .map(|p| !p.is_empty())
        .unwrap_or(false)
}

/// Check if recovery is pending.
pub fn is_restore_pending(session: &SessionState) -> bool {
    session.recovery.status.as_deref() == Some("restore_pending")
}

/// Check if TDD state allows production writes.
/// Production writes are allowed when tdd_state is in the "ready" set.
pub fn check_tdd_ready(session: &SessionState) -> bool {
    // If TDD is not required, always ready
    if session.node.tdd_required != Some(true) {
        return true;
    }

    let tdd_state = session.node.tdd_state.as_deref().unwrap_or("");

    matches!(
        tdd_state,
        "red_verified"
            | "green_pending"
            | "green_verified"
            | "refactor_pending"
            | "verified"
    )
}

/// Get the session.yaml file path.
#[allow(dead_code)]
pub fn session_path(workflow_dir: &Path) -> PathBuf {
    workflow_dir.join("session.yaml")
}

/// Get the task-graph.yaml file path.
#[allow(dead_code)]
pub fn task_graph_path(workflow_dir: &Path) -> PathBuf {
    workflow_dir.join("task-graph.yaml")
}

/// Get the journal.jsonl file path.
#[allow(dead_code)]
pub fn journal_path(workflow_dir: &Path) -> PathBuf {
    workflow_dir.join("journal.jsonl")
}
