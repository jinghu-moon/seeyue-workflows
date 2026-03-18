// tests/interaction_mcp.rs — P2-N4: MCP Interaction Tools Tests

use seeyue_mcp::tools::interaction_mcp::{
    run_list_interactions, run_read_interaction, run_resolve_interaction,
    ListInteractionsParams, ReadInteractionParams, ResolveInteractionParams,
    read_active_interactions,
};
use std::fs;
use tempfile::TempDir;

fn temp_workflow_dir() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("create tempdir");
    let workflow_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&workflow_dir).expect("create workflow dir");
    (tmp, workflow_dir)
}

/// Write a fake interaction request file for testing.
fn write_fake_request(workflow_dir: &std::path::Path, id: &str, kind: &str, status: &str, title: &str) {
    let requests_dir = workflow_dir.join("interactions").join("requests");
    fs::create_dir_all(&requests_dir).expect("create requests dir");
    let obj = serde_json::json!({
        "schema": 1,
        "interaction_id": id,
        "kind": kind,
        "status": status,
        "title": title,
        "message": title,
        "selection_mode": "boolean",
        "options": [],
        "comment_mode": "disabled",
        "presentation": {"mode": "text_menu", "color_profile": "auto", "theme": "auto"},
        "originating_request_id": "test-origin",
        "created_at": "2026-03-18T00:00:00Z",
    });
    let path = requests_dir.join(format!("{id}.json"));
    fs::write(&path, serde_json::to_string_pretty(&obj).unwrap() + "\n").expect("write fake request");
}

// ─── test_list_interactions_empty ────────────────────────────────────────────

#[test]
fn test_list_interactions_empty() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    // No requests dir at all
    let result = run_list_interactions(
        ListInteractionsParams { status: None },
        &workflow_dir,
    ).expect("list should succeed even with no requests dir");

    assert_eq!(result.total, 0, "empty dir should return 0 items");
    assert!(result.items.is_empty(), "items should be empty");
    assert_eq!(result.kind, "list");
}

// ─── test_list_interactions_filters_by_status ─────────────────────────────────

#[test]
fn test_list_interactions_filters_pending() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    write_fake_request(&workflow_dir, "ix-20260318-001", "approval_request", "pending", "Test 1");
    write_fake_request(&workflow_dir, "ix-20260318-002", "question_request", "answered", "Test 2");

    let result = run_list_interactions(
        ListInteractionsParams { status: Some("pending".into()) },
        &workflow_dir,
    ).expect("list pending");

    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].interaction_id, "ix-20260318-001");
}

#[test]
fn test_list_interactions_all() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    write_fake_request(&workflow_dir, "ix-20260318-001", "approval_request", "pending", "Test 1");
    write_fake_request(&workflow_dir, "ix-20260318-002", "question_request", "answered", "Test 2");

    let result = run_list_interactions(
        ListInteractionsParams { status: Some("all".into()) },
        &workflow_dir,
    ).expect("list all");

    assert_eq!(result.total, 2);
}

// ─── test_read_interaction_not_found ─────────────────────────────────────────

#[test]
fn test_read_interaction_not_found() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    let result = run_read_interaction(
        ReadInteractionParams { interaction_id: "ix-99999999-999".into() },
        &workflow_dir,
    ).expect("read should return not_found, not error");

    assert!(!result.found, "found must be false for missing ID");
    assert_eq!(result.kind, "not_found");
    assert!(result.data.is_none());
}

#[test]
fn test_read_interaction_found() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    write_fake_request(&workflow_dir, "ix-20260318-001", "approval_request", "pending", "Read me");

    let result = run_read_interaction(
        ReadInteractionParams { interaction_id: "ix-20260318-001".into() },
        &workflow_dir,
    ).expect("read should succeed");

    assert!(result.found);
    assert_eq!(result.kind, "found");
    let data = result.data.expect("data must be present");
    assert_eq!(data["title"].as_str().unwrap_or(""), "Read me");
}

// ─── test_resolve_interaction_writes_file ─────────────────────────────────────

#[test]
fn test_resolve_interaction_writes_file() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    write_fake_request(&workflow_dir, "ix-20260318-001", "approval_request", "pending", "Approve me");

    let result = run_resolve_interaction(
        ResolveInteractionParams {
            interaction_id:  "ix-20260318-001".into(),
            selected_option: "approve".into(),
            comment:         Some("Looks good".into()),
        },
        &workflow_dir,
    ).expect("resolve should succeed");

    assert!(result.resolved, "resolved must be true");
    assert_eq!(result.interaction_id, "ix-20260318-001");
    assert_eq!(result.kind, "resolved");

    // Verify response file was written
    let response_path = std::path::Path::new(&result.response_path);
    assert!(response_path.exists(), "response file must exist at {}", result.response_path);

    let content = fs::read_to_string(response_path).expect("read response file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse response JSON");
    assert_eq!(obj["interaction_id"].as_str().unwrap_or(""), "ix-20260318-001");
    assert_eq!(obj["selected_option"].as_str().unwrap_or(""), "approve");
    assert_eq!(obj["comment"].as_str().unwrap_or(""), "Looks good");
    assert_eq!(obj["resolver"].as_str().unwrap_or(""), "mcp");
}

// ─── test_read_active_interactions_missing ────────────────────────────────────

#[test]
fn test_read_active_interactions_missing() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    // No active.json — should return empty/null object
    let data = read_active_interactions(&workflow_dir);
    assert_eq!(data["active_id"], serde_json::Value::Null);
    assert_eq!(data["pending_count"], 0);
    assert_eq!(data["blocking_kind"], serde_json::Value::Null);
}

#[test]
fn test_read_active_interactions_present() {
    let (_tmp, workflow_dir) = temp_workflow_dir();
    let interactions_dir = workflow_dir.join("interactions");
    fs::create_dir_all(&interactions_dir).expect("create interactions dir");
    let active = serde_json::json!({
        "active_id": "ix-20260318-001",
        "pending_count": 1,
        "blocking_kind": "hard_gate",
        "blocking_reason": "Approval required",
    });
    fs::write(
        interactions_dir.join("active.json"),
        serde_json::to_string_pretty(&active).unwrap() + "\n",
    ).expect("write active.json");

    let data = read_active_interactions(&workflow_dir);
    assert_eq!(data["active_id"].as_str().unwrap_or(""), "ix-20260318-001");
    assert_eq!(data["pending_count"], 1);
    assert_eq!(data["blocking_kind"].as_str().unwrap_or(""), "hard_gate");
}
