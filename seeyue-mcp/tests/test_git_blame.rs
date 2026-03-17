// tests/test_git_blame.rs
//
// Tests for tools::git_blame::run_git_blame.
// Run: cargo test --test test_git_blame

use seeyue_mcp::tools::git_blame::{GitBlameParams, run_git_blame};

fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn params(path: &str) -> GitBlameParams {
    GitBlameParams { path: path.into(), start_line: None, end_line: None }
}

#[test]
fn test_git_blame_tracked_file_ok() {
    let result = run_git_blame(params("CLAUDE.md"), &workspace()).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_git_blame_has_lines() {
    let result = run_git_blame(params("CLAUDE.md"), &workspace()).unwrap();
    assert!(result.total > 0);
    assert_eq!(result.total, result.lines.len());
}

#[test]
fn test_git_blame_line_fields_populated() {
    let result = run_git_blame(params("CLAUDE.md"), &workspace()).unwrap();
    if let Some(l) = result.lines.first() {
        assert!(!l.hash.is_empty());
        assert!(!l.short.is_empty());
        assert!(!l.author.is_empty());
    }
}

#[test]
fn test_git_blame_line_range() {
    let result = run_git_blame(
        GitBlameParams { path: "CLAUDE.md".into(), start_line: Some(1), end_line: Some(3) },
        &workspace(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert!(result.total <= 3);
}

#[test]
fn test_git_blame_file_not_found_errors() {
    let err = run_git_blame(params("does_not_exist_xyz.md"), &workspace()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("FileNotFound") || msg.contains("NotFound"), "unexpected: {msg}");
}

#[test]
fn test_git_blame_non_git_dir_errors() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "hello").unwrap();
    let err = run_git_blame(params("f.txt"), tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("GitNotRepo") || msg.contains("GitError") || msg.contains("not a git"),
        "unexpected: {msg}"
    );
}

#[test]
fn test_git_blame_path_escape_blocked() {
    let err = run_git_blame(params("../../outside.txt"), &workspace()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("PathEscape") || msg.contains("outside"),
        "unexpected: {msg}"
    );
}
