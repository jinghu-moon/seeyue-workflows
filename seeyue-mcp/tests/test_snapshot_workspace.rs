// tests/test_snapshot_workspace.rs
//
// Tests for tools::snapshot_workspace::run_snapshot_workspace.
// Run: cargo test --test test_snapshot_workspace

use seeyue_mcp::tools::snapshot_workspace::{SnapshotWorkspaceParams, run_snapshot_workspace};

fn ws() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    // Create a few files to snapshot
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "world").unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub/c.txt"), "nested").unwrap();
    tmp
}

fn params(label: Option<&str>) -> SnapshotWorkspaceParams {
    SnapshotWorkspaceParams {
        label:           label.map(|s| s.into()),
        include_ignored: Some(true), // tempdir has no .gitignore
    }
}

#[test]
fn test_snapshot_returns_success() {
    let tmp = ws();
    let result = run_snapshot_workspace(params(Some("snap1")), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_snapshot_files_copied() {
    let tmp = ws();
    let result = run_snapshot_workspace(params(Some("snap2")), tmp.path()).unwrap();
    assert!(result.files_copied >= 3);
}

#[test]
fn test_snapshot_bytes_copied() {
    let tmp = ws();
    let result = run_snapshot_workspace(params(Some("snap3")), tmp.path()).unwrap();
    assert!(result.bytes_copied > 0);
}

#[test]
fn test_snapshot_path_returned() {
    let tmp = ws();
    let result = run_snapshot_workspace(params(Some("snap4")), tmp.path()).unwrap();
    assert!(result.snapshot_path.contains("snap4"));
    // Snapshot dir should exist
    assert!(tmp.path().join(&result.snapshot_path).exists());
}

#[test]
fn test_snapshot_files_are_readable() {
    let tmp = ws();
    let result = run_snapshot_workspace(params(Some("snap5")), tmp.path()).unwrap();
    let snap_dir = tmp.path().join(&result.snapshot_path);
    let content = std::fs::read_to_string(snap_dir.join("a.txt")).unwrap();
    assert_eq!(content, "hello");
}

#[test]
fn test_snapshot_duplicate_label_errors() {
    let tmp = ws();
    run_snapshot_workspace(params(Some("dup")), tmp.path()).unwrap();
    let err = run_snapshot_workspace(params(Some("dup")), tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("IoError") || msg.contains("already exists"), "unexpected: {msg}");
}

#[test]
fn test_snapshot_default_label() {
    let tmp = ws();
    let result = run_snapshot_workspace(
        SnapshotWorkspaceParams { label: None, include_ignored: Some(true) },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert!(!result.snapshot_path.is_empty());
}
