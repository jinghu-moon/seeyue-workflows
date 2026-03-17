// tests/test_git_diff_file.rs
//
// Tests for tools::git_diff_file::run_git_diff_file.
// Run: cargo test --test test_git_diff_file

use seeyue_mcp::tools::git_diff_file::{GitDiffFileParams, run_git_diff_file};

fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn params(path: &str) -> GitDiffFileParams {
    GitDiffFileParams {
        path:   path.into(),
        base:   Some("HEAD".into()),
        staged: Some(false),
    }
}

#[test]
fn test_diff_tracked_file_returns_ok() {
    let ws = workspace();
    let result = run_git_diff_file(params("CLAUDE.md"), &ws).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_diff_result_path_matches_input() {
    let ws = workspace();
    let result = run_git_diff_file(params("CLAUDE.md"), &ws).unwrap();
    assert!(result.path.contains("CLAUDE.md"));
}

#[test]
fn test_diff_base_defaults_to_head() {
    let ws = workspace();
    let result = run_git_diff_file(
        GitDiffFileParams { path: "CLAUDE.md".into(), base: None, staged: None },
        &ws,
    ).unwrap();
    assert_eq!(result.base, "HEAD");
}

#[test]
fn test_diff_staged_false_by_default() {
    let ws = workspace();
    let result = run_git_diff_file(
        GitDiffFileParams { path: "CLAUDE.md".into(), base: None, staged: None },
        &ws,
    ).unwrap();
    assert!(!result.staged);
}

#[test]
fn test_diff_non_git_dir_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = run_git_diff_file(params("any.txt"), tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("GitNotRepo") || msg.contains("GitError") || msg.contains("not a git"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_diff_path_escape_blocked() {
    let ws = workspace();
    let err = run_git_diff_file(params("../../outside.txt"), &ws).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("PathEscape") || msg.contains("outside"),
        "unexpected error: {msg}"
    );
}
