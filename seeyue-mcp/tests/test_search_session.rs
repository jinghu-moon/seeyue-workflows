// tests/test_search_session.rs
use seeyue_mcp::tools::search_session::{SearchSessionParams, run_search_session};
use std::fs;
use std::io::Write;

fn make_workflow_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn write_journal(dir: &std::path::Path) {
    let path = dir.join("journal.jsonl");
    let mut f = fs::File::create(&path).unwrap();
    let lines = [
        "{\"ts\":\"2026-03-17T00:01:00Z\",\"event\":\"write_recorded\",\"phase\":\"execute\",\"node_id\":\"node_1\",\"actor\":\"hook\",\"payload\":{\"path\":\"src/main.rs\"}}",
        "{\"ts\":\"2026-03-17T00:02:00Z\",\"event\":\"node_entered\",\"phase\":\"execute\",\"node_id\":\"node_2\",\"actor\":\"hook\",\"payload\":{\"name\":\"implement\"}}",
        "{\"ts\":\"2026-03-17T00:03:00Z\",\"event\":\"checkpoint_created\",\"phase\":\"plan\",\"node_id\":\"node_1\",\"actor\":\"hook\",\"payload\":{\"label\":\"before-execute\"}}",
        "{\"ts\":\"2026-03-17T00:04:00Z\",\"event\":\"write_recorded\",\"phase\":\"execute\",\"node_id\":\"node_2\",\"actor\":\"hook\",\"payload\":{\"path\":\"src/lib.rs\"}}",
    ];
    for line in &lines {
        writeln!(f, "{}", line).unwrap();
    }
}

fn params(query: &str) -> SearchSessionParams {
    SearchSessionParams {
        query:        query.into(),
        filter_event: None,
        filter_phase: None,
        filter_node:  None,
        limit:        None,
        sort_by:      None,
        since:        None,
        until:        None,
    }
}

#[test]
fn test_search_returns_success() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path());
    let result = run_search_session(params("write_recorded"), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_search_free_text_matches() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path());
    let result = run_search_session(params("write_recorded"), tmp.path()).unwrap();
    assert!(result.total >= 2);
    for e in &result.entries {
        assert_eq!(e.event, "write_recorded");
    }
}

#[test]
fn test_search_filter_event() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path());
    let result = run_search_session(
        SearchSessionParams {
            query:        "node".into(),
            filter_event: Some("node_entered".into()),
            filter_phase: None,
            filter_node:  None,
            limit:        None,
            sort_by:      None,
            since:        None,
            until:        None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.total >= 1);
    for e in &result.entries {
        assert_eq!(e.event, "node_entered");
    }
}

#[test]
fn test_search_filter_phase() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path());
    let result = run_search_session(
        SearchSessionParams {
            query:        "hook".into(),
            filter_event: None,
            filter_phase: Some("plan".into()),
            filter_node:  None,
            limit:        None,
            sort_by:      None,
            since:        None,
            until:        None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.total >= 1);
    for e in &result.entries {
        assert_eq!(e.phase.as_deref(), Some("plan"));
    }
}

#[test]
fn test_search_filter_node() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path());
    let result = run_search_session(
        SearchSessionParams {
            query:        "hook".into(),
            filter_event: None,
            filter_phase: None,
            filter_node:  Some("node_2".into()),
            limit:        None,
            sort_by:      None,
            since:        None,
            until:        None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.total >= 1);
    for e in &result.entries {
        assert_eq!(e.node_id.as_deref(), Some("node_2"));
    }
}

#[test]
fn test_search_empty_query_errors() {
    let tmp = make_workflow_dir();
    let err = run_search_session(params(""), tmp.path()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("MissingParameter") || msg.contains("query"), "unexpected: {}", msg);
}

#[test]
fn test_search_no_journal_returns_empty() {
    let tmp = make_workflow_dir();
    let result = run_search_session(params("anything"), tmp.path()).unwrap();
    assert_eq!(result.total, 0);
    assert!(result.entries.is_empty());
}

#[test]
fn test_search_limit_respected() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path());
    let result = run_search_session(
        SearchSessionParams {
            query:        "hook".into(),
            filter_event: None,
            filter_phase: None,
            filter_node:  None,
            limit:        Some(1),
            sort_by:      None,
            since:        None,
            until:        None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.entries.len() <= 1);
}
