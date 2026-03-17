// tests/test_compact_journal.rs
use seeyue_mcp::tools::compact_journal::{CompactJournalParams, run_compact_journal};
use std::fs;
use std::io::Write;

fn make_workflow_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn write_journal(dir: &std::path::Path, lines: usize) {
    let path = dir.join("journal.jsonl");
    let mut f = fs::File::create(&path).unwrap();
    for i in 0..lines {
        let line = format!(
            "{{\"ts\":\"2026-03-17T00:00:00Z\",\"event\":\"write_recorded\",\"actor\":\"hook\",\"run_id\":\"r1\",\"node_id\":\"n{}\"}}",
            i
        );
        writeln!(f, "{}", line).unwrap();
    }
}

fn params(max_entries: usize, summarize: bool) -> CompactJournalParams {
    CompactJournalParams { max_entries: Some(max_entries), summarize }
}

#[test]
fn test_compact_already_compact() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path(), 10);
    let result = run_compact_journal(params(200, false), tmp.path()).unwrap();
    assert_eq!(result.kind, "already_compact");
    assert_eq!(result.archived, 0);
    assert_eq!(result.retained, 10);
}

#[test]
fn test_compact_archives_old_entries() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path(), 50);
    let result = run_compact_journal(params(20, false), tmp.path()).unwrap();
    assert_eq!(result.kind, "compacted");
    assert_eq!(result.archived, 30);
    assert_eq!(result.retained, 20);
    assert_eq!(result.total_before, 50);
}

#[test]
fn test_compact_archive_file_created() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path(), 50);
    let result = run_compact_journal(params(10, false), tmp.path()).unwrap();
    let archive_name = result.archive_file.unwrap();
    let archive_path = tmp.path().join(&archive_name);
    assert!(archive_path.exists(), "archive file should exist: {}", archive_name);
    let content = fs::read_to_string(&archive_path).unwrap();
    assert_eq!(content.lines().count(), 40);
}

#[test]
fn test_compact_journal_retained_lines_correct() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path(), 50);
    run_compact_journal(params(15, false), tmp.path()).unwrap();
    let journal = fs::read_to_string(tmp.path().join("journal.jsonl")).unwrap();
    assert_eq!(journal.lines().count(), 15);
}

#[test]
fn test_compact_empty_journal() {
    let tmp = make_workflow_dir();
    let result = run_compact_journal(params(200, false), tmp.path()).unwrap();
    assert_eq!(result.kind, "already_compact");
    assert_eq!(result.total_before, 0);
}

#[test]
fn test_compact_summarize_appends_to_session() {
    let tmp = make_workflow_dir();
    write_journal(tmp.path(), 50);
    fs::write(
        tmp.path().join("session.yaml"),
        "run_id: r1\nphase:\n  id: execute\n",
    ).unwrap();
    let result = run_compact_journal(params(10, true), tmp.path()).unwrap();
    assert!(result.summary_written);
    let session = fs::read_to_string(tmp.path().join("session.yaml")).unwrap();
    assert!(session.contains("compact_journal"));
}
