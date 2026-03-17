// tests/test_session_summary.rs
//
// Tests for tools::session_summary::run_session_summary.
// Run: cargo test --test test_session_summary

use seeyue_mcp::tools::session_summary::run_session_summary;
use seeyue_mcp::storage::checkpoint::CheckpointStore;
use seeyue_mcp::workflow::state::{SessionState, PhaseState, save_session};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn store(dir: &std::path::Path) -> CheckpointStore {
    CheckpointStore::open("test", &dir.join(".seeyue")).unwrap()
}

#[test]
fn test_no_session_file_returns_not_found() {
    let tmp = ws();
    let cp = store(tmp.path());
    let result = run_session_summary(tmp.path(), &cp).unwrap();
    assert_eq!(result.status, "SESSION_NOT_FOUND");
}

#[test]
fn test_no_session_active_node_is_none() {
    let tmp = ws();
    let cp = store(tmp.path());
    let result = run_session_summary(tmp.path(), &cp).unwrap();
    assert!(result.active_node.is_none());
}

#[test]
fn test_no_session_budget_exceeded_false() {
    let tmp = ws();
    let cp = store(tmp.path());
    let result = run_session_summary(tmp.path(), &cp).unwrap();
    assert!(!result.loop_budget.exceeded);
}

#[test]
fn test_no_session_checkpoint_count_zero() {
    let tmp = ws();
    let cp = store(tmp.path());
    // SESSION_NOT_FOUND branch always returns 0 regardless of store contents
    let result = run_session_summary(tmp.path(), &cp).unwrap();
    assert_eq!(result.checkpoint_count, 0);
}

#[test]
fn test_checkpoint_count_reflects_store_with_session() {
    let tmp = ws();
    let cp = store(tmp.path());
    // Capture two snapshots
    let f = tmp.path().join("f.txt");
    std::fs::write(&f, "x").unwrap();
    cp.capture(&f, "id-1", "tool").unwrap();
    cp.capture(&f, "id-2", "tool").unwrap();
    // Write a valid session.yaml so we get the "ok" path
    let mut state = SessionState::default();
    state.run_id = Some("run-checkpoint-test".to_string());
    save_session(tmp.path(), &state).unwrap();
    let result = run_session_summary(tmp.path(), &cp).unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.checkpoint_count, 2);
}

#[test]
fn test_with_session_yaml_returns_ok() {
    let tmp = ws();
    let cp = store(tmp.path());
    // Use save_session to write a properly serialized session.yaml
    let mut state = SessionState::default();
    state.run_id = Some("run-001".to_string());
    state.phase = PhaseState { id: Some("coding".to_string()), name: None, status: Some("active".to_string()) };
    save_session(tmp.path(), &state).unwrap();
    let result = run_session_summary(tmp.path(), &cp).unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.run_id.as_deref(), Some("run-001"));
}
