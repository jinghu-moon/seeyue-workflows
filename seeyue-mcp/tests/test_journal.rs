// tests/test_journal.rs
//
// Functional tests for workflow::journal: append_event, read_recent, JournalEvent builder.
// Uses tempfile for isolated workflow dirs.
// Run: cargo test --test test_journal

use seeyue_mcp::workflow::journal::{append_event, read_recent, JournalEvent};

// ─── JournalEvent builder ─────────────────────────────────────────────────────

#[test]
fn test_event_new_has_ts_and_trace_id() {
    let ev = JournalEvent::new("test_event", "agent");
    assert_eq!(ev.event, "test_event");
    assert_eq!(ev.actor, "agent");
    assert!(!ev.ts.is_empty(), "ts should be set");
    assert!(ev.trace_id.is_some(), "trace_id should be generated");
}

#[test]
fn test_event_with_run_id() {
    let ev = JournalEvent::new("e", "a").with_run_id("run-001");
    assert_eq!(ev.run_id.as_deref(), Some("run-001"));
}

#[test]
fn test_event_with_phase() {
    let ev = JournalEvent::new("e", "a").with_phase("P1");
    assert_eq!(ev.phase.as_deref(), Some("P1"));
}

#[test]
fn test_event_with_node_id() {
    let ev = JournalEvent::new("e", "a").with_node_id("N1");
    assert_eq!(ev.node_id.as_deref(), Some("N1"));
}

#[test]
fn test_event_with_payload() {
    let ev = JournalEvent::new("e", "a")
        .with_payload(serde_json::json!({"key": "value"}));
    let payload = ev.payload.unwrap();
    assert_eq!(payload["key"], "value");
}

#[test]
fn test_event_builder_chain() {
    let ev = JournalEvent::new("write_evidence", "sy-hook")
        .with_run_id("wf-001")
        .with_phase("P1")
        .with_node_id("N2")
        .with_payload(serde_json::json!({"file": "src/main.rs"}));

    assert_eq!(ev.run_id.as_deref(), Some("wf-001"));
    assert_eq!(ev.phase.as_deref(), Some("P1"));
    assert_eq!(ev.node_id.as_deref(), Some("N2"));
    assert!(ev.payload.is_some());
}

// ─── append_event ─────────────────────────────────────────────────────────────

#[test]
fn test_append_creates_journal_file() {
    let tmp = tempfile::tempdir().unwrap();
    let ev = JournalEvent::new("session_start", "test");
    append_event(tmp.path(), ev).unwrap();
    assert!(tmp.path().join("journal.jsonl").exists());
}

#[test]
fn test_append_writes_valid_jsonl() {
    let tmp = tempfile::tempdir().unwrap();
    let ev = JournalEvent::new("test_event", "agent").with_run_id("r1");
    append_event(tmp.path(), ev).unwrap();

    let content = std::fs::read_to_string(tmp.path().join("journal.jsonl")).unwrap();
    let line = content.lines().next().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(parsed["event"], "test_event");
    assert_eq!(parsed["actor"], "agent");
    assert_eq!(parsed["run_id"], "r1");
}

#[test]
fn test_append_multiple_events() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..5 {
        let ev = JournalEvent::new(format!("event_{i}"), "test");
        append_event(tmp.path(), ev).unwrap();
    }
    let content = std::fs::read_to_string(tmp.path().join("journal.jsonl")).unwrap();
    assert_eq!(content.lines().count(), 5, "should have 5 lines");
}

#[test]
fn test_append_creates_parent_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("deep").join("nested");
    let ev = JournalEvent::new("e", "a");
    append_event(&nested, ev).unwrap();
    assert!(nested.join("journal.jsonl").exists());
}

#[test]
fn test_append_idempotent_grows_file() {
    let tmp = tempfile::tempdir().unwrap();
    append_event(tmp.path(), JournalEvent::new("e1", "a")).unwrap();
    let size1 = std::fs::metadata(tmp.path().join("journal.jsonl")).unwrap().len();
    append_event(tmp.path(), JournalEvent::new("e2", "a")).unwrap();
    let size2 = std::fs::metadata(tmp.path().join("journal.jsonl")).unwrap().len();
    assert!(size2 > size1, "file should grow with each append");
}

// ─── read_recent ──────────────────────────────────────────────────────────────

#[test]
fn test_read_recent_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    // No journal.jsonl yet — should return empty or error gracefully
    let result = read_recent(tmp.path(), 10);
    // Either Ok("") or Ok with no lines — should not panic
    match result {
        Ok(s) => assert!(s.is_empty() || s.lines().count() == 0),
        Err(_) => {} // acceptable if file not found
    }
}

#[test]
fn test_read_recent_returns_last_n_lines() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..10 {
        append_event(tmp.path(), JournalEvent::new(format!("ev_{i}"), "a")).unwrap();
    }
    let result = read_recent(tmp.path(), 3).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 3, "should return last 3 lines");
}

#[test]
fn test_read_recent_max_larger_than_total() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..4 {
        append_event(tmp.path(), JournalEvent::new(format!("ev_{i}"), "a")).unwrap();
    }
    let result = read_recent(tmp.path(), 100).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 4, "should return all 4 lines when max > total");
}

#[test]
fn test_read_recent_lines_are_valid_json() {
    let tmp = tempfile::tempdir().unwrap();
    append_event(tmp.path(), JournalEvent::new("check", "tester").with_phase("P1")).unwrap();
    let result = read_recent(tmp.path(), 5).unwrap();
    for line in result.lines() {
        let v: serde_json::Value = serde_json::from_str(line)
            .expect("each line should be valid JSON");
        assert!(v["event"].is_string());
    }
}

#[test]
fn test_read_recent_returns_most_recent_events() {
    let tmp = tempfile::tempdir().unwrap();
    // Write events 0..9, then read last 3 — should contain ev_7, ev_8, ev_9
    for i in 0..10u32 {
        append_event(tmp.path(), JournalEvent::new(format!("ev_{i}"), "a")).unwrap();
    }
    let result = read_recent(tmp.path(), 3).unwrap();
    assert!(result.contains("ev_9"), "last event should be present");
    assert!(result.contains("ev_8"), "second-to-last should be present");
    assert!(result.contains("ev_7"), "third-to-last should be present");
    assert!(!result.contains("ev_0"), "oldest events should be excluded");
}
