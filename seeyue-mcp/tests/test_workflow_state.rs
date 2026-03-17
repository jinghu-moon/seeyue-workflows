// tests/test_workflow_state.rs
//
// Functional tests for workflow::state query helpers.
// Run: cargo test --test test_workflow_state

use rstest::rstest;

use seeyue_mcp::workflow::state::{
    check_loop_budget, check_tdd_ready, has_pending_approvals, is_restore_pending,
    ApprovalState, LoopBudget, NodeState, PhaseState, RecoveryState, SessionState,
};

// ─── Fixture ─────────────────────────────────────────────────────────────────

fn base_session() -> SessionState {
    SessionState {
        schema_version: Some(1),
        run_id: Some("test".to_string()),
        phase: PhaseState { id: Some("P1".to_string()), name: None, status: Some("active".to_string()) },
        node: NodeState {
            id: Some("N1".to_string()), name: None, status: None,
            state: None, tdd_required: None, tdd_state: None,
            tdd_exception: None, target: None, test_contract: None,
            owner_persona: None, phase_id: None,
        },
        loop_budget: LoopBudget {
            max: Some(100), used: Some(5), exhausted: Some(false),
            max_nodes: None, consumed_nodes: None,
            max_failures: None, consumed_failures: None,
            max_pending_approvals: None,
            max_context_utilization: None, current_context_utilization: None,
            max_rework_cycles: None, consumed_rework_cycles: None,
        },
        approvals: ApprovalState { pending: None, grants: None },
        recovery: RecoveryState { status: None, restore_reason: None, last_checkpoint_id: None },
    }
}

// ─── check_loop_budget ───────────────────────────────────────────────────────

#[test]
fn test_budget_ok() {
    assert!(check_loop_budget(&base_session()).is_none());
}

#[test]
fn test_budget_legacy_exhausted_flag() {
    let mut s = base_session();
    s.loop_budget.exhausted = Some(true);
    assert!(check_loop_budget(&s).is_some());
}

#[test]
fn test_budget_legacy_used_ge_max() {
    let mut s = base_session();
    s.loop_budget.used = Some(100);
    s.loop_budget.max = Some(100);
    assert!(check_loop_budget(&s).is_some());
}

#[rstest]
#[case(5, 5)]   // consumed == max → exceeded
#[case(6, 5)]   // consumed > max → exceeded
fn test_budget_v4_nodes_exceeded(#[case] consumed: u32, #[case] max: u32) {
    let mut s = base_session();
    s.loop_budget.max_nodes = Some(max);
    s.loop_budget.consumed_nodes = Some(consumed);
    assert!(check_loop_budget(&s).is_some());
}

#[test]
fn test_budget_v4_nodes_ok() {
    let mut s = base_session();
    s.loop_budget.max_nodes = Some(10);
    s.loop_budget.consumed_nodes = Some(9);
    assert!(check_loop_budget(&s).is_none());
}

#[test]
fn test_budget_v4_failures_exceeded() {
    let mut s = base_session();
    s.loop_budget.max_failures = Some(3);
    s.loop_budget.consumed_failures = Some(3);
    assert!(check_loop_budget(&s).is_some());
}

#[test]
fn test_budget_v4_context_exceeded() {
    let mut s = base_session();
    s.loop_budget.max_context_utilization = Some(0.9);
    s.loop_budget.current_context_utilization = Some(0.95);
    assert!(check_loop_budget(&s).is_some());
}

#[test]
fn test_budget_v4_rework_exceeded() {
    let mut s = base_session();
    s.loop_budget.max_rework_cycles = Some(2);
    s.loop_budget.consumed_rework_cycles = Some(2);
    assert!(check_loop_budget(&s).is_some());
}

// ─── has_pending_approvals ────────────────────────────────────────────────────

#[test]
fn test_no_pending_approvals() {
    assert!(!has_pending_approvals(&base_session()));
}

#[test]
fn test_empty_pending_list() {
    let mut s = base_session();
    s.approvals.pending = Some(vec![]);
    assert!(!has_pending_approvals(&s));
}

#[test]
fn test_has_pending_approvals() {
    let mut s = base_session();
    s.approvals.pending = Some(vec![serde_yaml::Value::String("approval-1".to_string())]);
    assert!(has_pending_approvals(&s));
}

// ─── is_restore_pending ───────────────────────────────────────────────────────

#[test]
fn test_not_restore_pending() {
    assert!(!is_restore_pending(&base_session()));
}

#[test]
fn test_is_restore_pending() {
    let mut s = base_session();
    s.recovery.status = Some("restore_pending".to_string());
    assert!(is_restore_pending(&s));
}

#[test]
fn test_other_recovery_status_not_pending() {
    let mut s = base_session();
    s.recovery.status = Some("recovered".to_string());
    assert!(!is_restore_pending(&s));
}

// ─── check_tdd_ready ─────────────────────────────────────────────────────────

#[test]
fn test_tdd_not_required_always_ready() {
    let mut s = base_session();
    s.node.tdd_required = Some(false);
    s.node.tdd_state = Some("no_tests".to_string());
    assert!(check_tdd_ready(&s));
}

#[rstest]
#[case("red_verified",     true)]
#[case("green_pending",    true)]
#[case("green_verified",   true)]
#[case("refactor_pending", true)]
#[case("verified",         true)]
#[case("no_tests",         false)]
#[case("red_written",      false)]
fn test_tdd_state_readiness(#[case] state: &str, #[case] expected: bool) {
    let mut s = base_session();
    s.node.tdd_required = Some(true);
    s.node.tdd_state = Some(state.to_string());
    assert_eq!(check_tdd_ready(&s), expected, "tdd_state '{}'", state);
}
