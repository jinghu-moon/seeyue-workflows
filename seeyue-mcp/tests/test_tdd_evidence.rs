// tests/test_tdd_evidence.rs
use seeyue_mcp::tools::tdd_evidence::{TddEvidenceParams, run_tdd_evidence};
use std::io::Write;

fn write_journal(dir: &std::path::Path) {
    let path = dir.join("journal.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    let lines = [
        "{\"ts\":\"2026-03-18T00:01:00Z\",\"event\":\"node_entered\",\"node_id\":\"n1\",\"run_id\":\"r1\",\"actor\":\"hook\",\"payload\":{}}",
        "{\"ts\":\"2026-03-18T00:02:00Z\",\"event\":\"write_recorded\",\"node_id\":\"n1\",\"run_id\":\"r1\",\"actor\":\"hook\",\"payload\":{\"path\":\"src/lib.rs\",\"tdd_state\":\"red_verified\"}}",
        "{\"ts\":\"2026-03-18T00:03:00Z\",\"event\":\"write_recorded\",\"node_id\":\"n1\",\"run_id\":\"r1\",\"actor\":\"hook\",\"payload\":{\"path\":\"src/main.rs\",\"tdd_state\":\"green_verified\"}}",
        "{\"ts\":\"2026-03-18T00:04:00Z\",\"event\":\"node_entered\",\"node_id\":\"n2\",\"run_id\":\"r1\",\"actor\":\"hook\",\"payload\":{}}",
    ];
    for line in &lines {
        writeln!(f, "{}", line).unwrap();
    }
}

#[test]
fn test_tdd_evidence_all_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    write_journal(tmp.path());
    let result = run_tdd_evidence(
        TddEvidenceParams { node_id: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.total, 2);
}

#[test]
fn test_tdd_evidence_filter_node() {
    let tmp = tempfile::tempdir().unwrap();
    write_journal(tmp.path());
    let result = run_tdd_evidence(
        TddEvidenceParams { node_id: Some("n1".into()) },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.nodes[0].node_id, "n1");
    assert!(result.nodes[0].red_verified);
    assert!(result.nodes[0].green_verified);
    assert!(result.nodes[0].files_written.len() >= 2);
}

#[test]
fn test_tdd_evidence_empty_journal() {
    let tmp = tempfile::tempdir().unwrap();
    let result = run_tdd_evidence(
        TddEvidenceParams { node_id: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "empty");
    assert_eq!(result.total, 0);
}
