// src/tools/session_summary.rs
//
// Returns a structured summary of the current workflow session:
// active node, phase, loop budget consumption, modified files, checkpoint count.

use std::path::Path;

use serde::Serialize;

use crate::error::ToolError;
use crate::workflow::state;

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SessionSummaryResult {
    pub status:        String,   // "ok" | "SESSION_NOT_FOUND"
    pub run_id:        Option<String>,
    pub phase:         Option<String>,
    pub phase_status:  Option<String>,
    pub active_node:   Option<NodeSummary>,
    pub loop_budget:   BudgetSummary,
    pub pending_approvals: u32,
    pub checkpoint_count:  u32,
    pub recovery_status:   Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NodeSummary {
    pub id:           Option<String>,
    pub name:         Option<String>,
    pub status:       Option<String>,
    pub tdd_state:    Option<String>,
    pub tdd_required: Option<bool>,
    pub targets:      Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct BudgetSummary {
    pub consumed_nodes:    Option<u32>,
    pub max_nodes:         Option<u32>,
    pub consumed_failures: Option<u32>,
    pub max_failures:      Option<u32>,
    pub pending_approvals: u32,
    pub max_pending_approvals: Option<u32>,
    pub exceeded:          bool,
    pub exceeded_reason:   Option<String>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_session_summary(
    workflow_dir: &Path,
    checkpoint_store: &crate::checkpoint::CheckpointStore,
) -> Result<SessionSummaryResult, ToolError> {
    let session_path = workflow_dir.join("session.yaml");
    if !session_path.exists() {
        return Ok(SessionSummaryResult {
            status:            "SESSION_NOT_FOUND".to_string(),
            run_id:            None,
            phase:             None,
            phase_status:      None,
            active_node:       None,
            loop_budget:       BudgetSummary {
                consumed_nodes:        None,
                max_nodes:             None,
                consumed_failures:     None,
                max_failures:          None,
                pending_approvals:     0,
                max_pending_approvals: None,
                exceeded:              false,
                exceeded_reason:       None,
            },
            pending_approvals: 0,
            checkpoint_count:  0,
            recovery_status:   None,
        });
    }

    let session = state::load_session(workflow_dir);

    let pending_approvals = session
        .approvals
        .pending
        .as_ref()
        .map(|v| v.len() as u32)
        .unwrap_or(0);

    let budget_exceeded = state::check_loop_budget(&session);
    let b = &session.loop_budget;

    let budget = BudgetSummary {
        consumed_nodes:        b.consumed_nodes.or(b.used),
        max_nodes:             b.max_nodes.or(b.max),
        consumed_failures:     b.consumed_failures,
        max_failures:          b.max_failures,
        pending_approvals,
        max_pending_approvals: b.max_pending_approvals,
        exceeded:              budget_exceeded.is_some(),
        exceeded_reason:       budget_exceeded,
    };

    let checkpoint_count = checkpoint_store.list().len() as u32;

    Ok(SessionSummaryResult {
        status: "ok".to_string(),
        run_id: session.run_id,
        phase:  session.phase.id,
        phase_status: session.phase.status,
        active_node: Some(NodeSummary {
            id:           session.node.id,
            name:         session.node.name,
            status:       session.node.status,
            tdd_state:    session.node.tdd_state,
            tdd_required: session.node.tdd_required,
            targets:      session.node.target,
        }),
        loop_budget:       budget,
        pending_approvals,
        checkpoint_count,
        recovery_status:   session.recovery.status,
    })
}
