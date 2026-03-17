// tests/test_git_log.rs
//
// Tests for tools::git_log::run_git_log.
// Run: cargo test --test test_git_log

use seeyue_mcp::tools::git_log::{GitLogParams, run_git_log};

fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn test_git_log_returns_success() {
    let result = run_git_log(
        GitLogParams { limit: Some(5), path: None, since: None },
        &workspace(),
    ).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_git_log_respects_limit() {
    let result = run_git_log(
        GitLogParams { limit: Some(3), path: None, since: None },
        &workspace(),
    ).unwrap();
    assert!(result.total <= 3);
    assert!(result.commits.len() <= 3);
}

#[test]
fn test_git_log_total_matches_commits_len() {
    let result = run_git_log(
        GitLogParams { limit: Some(10), path: None, since: None },
        &workspace(),
    ).unwrap();
    assert_eq!(result.total, result.commits.len());
}

#[test]
fn test_git_log_commit_fields_populated() {
    let result = run_git_log(
        GitLogParams { limit: Some(1), path: None, since: None },
        &workspace(),
    ).unwrap();
    if let Some(c) = result.commits.first() {
        assert!(!c.hash.is_empty());
        assert!(!c.short.is_empty());
        assert!(!c.author.is_empty());
        assert!(!c.date.is_empty());
    }
}

#[test]
fn test_git_log_non_git_dir_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = run_git_log(
        GitLogParams { limit: None, path: None, since: None },
        tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("GitNotRepo") || msg.contains("GitError") || msg.contains("not a git"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_git_log_path_filter_works() {
    let result = run_git_log(
        GitLogParams { limit: Some(10), path: Some("CLAUDE.md".into()), since: None },
        &workspace(),
    ).unwrap();
    // result may be empty if file has no commits, but should not error
    assert_eq!(result.kind, "success");
}

#[test]
fn test_git_log_path_escape_blocked() {
    let err = run_git_log(
        GitLogParams { limit: None, path: Some("../../outside".into()), since: None },
        &workspace(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("PathEscape") || msg.contains("outside"),
        "unexpected error: {msg}"
    );
}
