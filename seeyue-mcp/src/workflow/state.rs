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
    pub tdd_exception: Option<serde_yaml::Value>,
    #[serde(default)]
    pub test_contract: Option<serde_yaml::Value>,
    /// Persona executing this node (e.g. "author", "spec_reviewer").
    /// Read from session.yaml node.owner_persona.
    #[serde(default)]
    pub owner_persona: Option<String>,
    /// phase_id this node belongs to (for cross-field consistency check).
    #[serde(default)]
    pub phase_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoopBudget {
    // Legacy fields (kept for backward compatibility with existing session.yaml files)
    #[serde(default)]
    pub max: Option<u32>,
    #[serde(default)]
    pub used: Option<u32>,
    #[serde(default)]
    pub exhausted: Option<bool>,

    // V4 six-metric budget (architecture-v4.md §6.1)
    #[serde(default)]
    pub max_nodes: Option<u32>,
    #[serde(default)]
    pub consumed_nodes: Option<u32>,
    #[serde(default)]
    pub max_failures: Option<u32>,
    #[serde(default)]
    pub consumed_failures: Option<u32>,
    #[serde(default)]
    pub max_pending_approvals: Option<u32>,
    #[serde(default)]
    pub max_context_utilization: Option<f32>,
    #[serde(default)]
    pub current_context_utilization: Option<f32>,
    #[serde(default)]
    pub max_rework_cycles: Option<u32>,
    #[serde(default)]
    pub consumed_rework_cycles: Option<u32>,
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
    #[serde(default)]
    pub last_checkpoint_id: Option<String>,
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

/// Check loop budget against all six V4 metrics.
/// Returns `Some(BudgetExceeded { metric, consumed, max })` if any limit is breached.
/// Legacy `max/used/exhausted` fields are checked for backward compatibility.
pub fn check_loop_budget(session: &SessionState) -> Option<String> {
    let b = &session.loop_budget;

    // Legacy exhausted flag
    if b.exhausted == Some(true) {
        return Some("budget_exceeded: nodes (legacy exhausted flag)".to_string());
    }

    // Legacy max/used
    if let (Some(max), Some(used)) = (b.max, b.used) {
        if used >= max {
            return Some(format!(
                "budget_exceeded: nodes ({}/{} iterations used)",
                used, max
            ));
        }
    }

    // V4: max_nodes
    if let (Some(max), Some(consumed)) = (b.max_nodes, b.consumed_nodes) {
        if consumed >= max {
            return Some(format!(
                "budget_exceeded: nodes ({}/{} nodes consumed)",
                consumed, max
            ));
        }
    }

    // V4: max_failures
    if let (Some(max), Some(consumed)) = (b.max_failures, b.consumed_failures) {
        if consumed >= max {
            return Some(format!(
                "budget_exceeded: failures ({}/{} failures)",
                consumed, max
            ));
        }
    }

    // V4: max_pending_approvals — check against current pending count
    if let Some(max) = b.max_pending_approvals {
        let pending = session
            .approvals
            .pending
            .as_ref()
            .map(|v| v.len() as u32)
            .unwrap_or(0);
        if pending >= max {
            return Some(format!(
                "budget_exceeded: approvals ({}/{} pending approvals)",
                pending, max
            ));
        }
    }

    // V4: max_context_utilization
    if let (Some(max), Some(current)) =
        (b.max_context_utilization, b.current_context_utilization)
    {
        if current >= max {
            return Some(format!(
                "budget_exceeded: context ({:.0}%/{:.0}% context utilized)",
                current * 100.0,
                max * 100.0
            ));
        }
    }

    // V4: max_rework_cycles
    if let (Some(max), Some(consumed)) = (b.max_rework_cycles, b.consumed_rework_cycles) {
        if consumed >= max {
            return Some(format!(
                "budget_exceeded: rework ({}/{} rework cycles)",
                consumed, max
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
