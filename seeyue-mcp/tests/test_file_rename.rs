// tests/test_file_rename.rs
//
// Tests for tools::file_rename::run_file_rename.
// Run: cargo test --test test_file_rename

use seeyue_mcp::tools::file_rename::{FileRenameParams, run_file_rename};
use seeyue_mcp::storage::checkpoint::CheckpointStore;

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn store(dir: &std::path::Path) -> CheckpointStore {
    CheckpointStore::open("test", &dir.join(".seeyue")).unwrap()
}

#[test]
fn test_file_rename_basic() {
    let tmp = ws();
    std::fs::write(tmp.path().join("old.txt"), "content").unwrap();
    let cp = store(tmp.path());
    let result = run_file_rename(
        FileRenameParams { old_path: "old.txt".into(), new_path: "new.txt".into() },
        &cp, tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert!(!tmp.path().join("old.txt").exists());
    assert!(tmp.path().join("new.txt").exists());
}

#[test]
fn test_file_rename_result_paths_match() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "x").unwrap();
    let cp = store(tmp.path());
    let result = run_file_rename(
        FileRenameParams { old_path: "a.txt".into(), new_path: "b.txt".into() },
        &cp, tmp.path(),
    ).unwrap();
    assert_eq!(result.old_path, "a.txt");
    assert_eq!(result.new_path, "b.txt");
}

#[test]
fn test_file_rename_checkpoint_id_returned() {
    let tmp = ws();
    std::fs::write(tmp.path().join("x.txt"), "data").unwrap();
    let cp = store(tmp.path());
    let result = run_file_rename(
        FileRenameParams { old_path: "x.txt".into(), new_path: "y.txt".into() },
        &cp, tmp.path(),
    ).unwrap();
    assert!(!result.checkpoint_id.is_empty());
}

#[test]
fn test_file_rename_source_not_found_errors() {
    let tmp = ws();
    let cp = store(tmp.path());
    let err = run_file_rename(
        FileRenameParams { old_path: "ghost.txt".into(), new_path: "new.txt".into() },
        &cp, tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("FileNotFound") || msg.contains("NotFound"), "unexpected: {msg}");
}

#[test]
fn test_file_rename_dest_exists_errors() {
    let tmp = ws();
    std::fs::write(tmp.path().join("src.txt"), "a").unwrap();
    std::fs::write(tmp.path().join("dst.txt"), "b").unwrap();
    let cp = store(tmp.path());
    let err = run_file_rename(
        FileRenameParams { old_path: "src.txt".into(), new_path: "dst.txt".into() },
        &cp, tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("IoError") || msg.contains("already exists"), "unexpected: {msg}");
}

#[test]
fn test_file_rename_path_escape_blocked() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "data").unwrap();
    let cp = store(tmp.path());
    let err = run_file_rename(
        FileRenameParams { old_path: "../../outside.txt".into(), new_path: "new.txt".into() },
        &cp, tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("PathEscape") || msg.contains("outside"), "unexpected: {msg}");
}

#[test]
fn test_file_rename_creates_parent_dir() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "data").unwrap();
    let cp = store(tmp.path());
    let result = run_file_rename(
        FileRenameParams { old_path: "f.txt".into(), new_path: "subdir/f.txt".into() },
        &cp, tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert!(tmp.path().join("subdir/f.txt").exists());
}
