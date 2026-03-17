// tests/test_git_status.rs
//
// Tests for tools::git_status::run_git_status.
// Run: cargo test --test test_git_status

use seeyue_mcp::tools::git_status::run_git_status;

#[test]
fn test_git_status_in_git_repo() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let result = run_git_status(workspace).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_git_status_branch_accessible() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let result = run_git_status(workspace).unwrap();
    let _ = result.branch;
}

#[test]
fn test_git_status_file_vecs_accessible() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let result = run_git_status(workspace).unwrap();
    let _ = result.modified.len();
    let _ = result.added.len();
    let _ = result.deleted.len();
    let _ = result.untracked.len();
    let _ = result.staged.len();
    let _ = result.conflicts.len();
}

#[test]
fn test_git_status_non_git_dir_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = run_git_status(tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("GitNotRepo") || msg.contains("GitError") || msg.contains("not a git"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_git_status_clean_field_accessible() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let result = run_git_status(workspace).unwrap();
    let _ = result.clean;
}

#[test]
fn test_git_status_clean_consistent_with_files() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let result = run_git_status(workspace).unwrap();
    let total = result.modified.len()
        + result.added.len()
        + result.deleted.len()
        + result.untracked.len()
        + result.staged.len()
        + result.conflicts.len();
    assert_eq!(result.clean, total == 0);
}
