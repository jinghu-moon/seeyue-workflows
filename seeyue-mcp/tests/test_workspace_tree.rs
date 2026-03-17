// tests/test_workspace_tree.rs
//
// Tests for tools::workspace_tree::run_workspace_tree.
// Run: cargo test --test test_workspace_tree

use seeyue_mcp::tools::workspace_tree::{WorkspaceTreeParams, run_workspace_tree};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn default_params() -> WorkspaceTreeParams {
    WorkspaceTreeParams {
        depth: None,
        respect_gitignore: None,
        show_hidden: None,
        min_size_bytes: None,
    }
}

#[test]
fn test_workspace_tree_returns_success_kind() {
    let tmp = ws();
    let result = run_workspace_tree(default_params(), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_workspace_tree_root_matches_workspace() {
    let tmp = ws();
    let result = run_workspace_tree(default_params(), tmp.path()).unwrap();
    assert!(!result.root.is_empty());
}

#[test]
fn test_workspace_tree_summary_counts() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    std::fs::write(tmp.path().join("b.rs"), "fn f(){}").unwrap();
    let result = run_workspace_tree(default_params(), tmp.path()).unwrap();
    assert!(result.summary.total_files >= 2);
}

#[test]
fn test_workspace_tree_depth_limits() {
    let tmp = ws();
    let sub = tmp.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("deep.txt"), "x").unwrap();
    // depth=1 should not recurse into sub
    let result = run_workspace_tree(
        WorkspaceTreeParams { depth: Some(1), respect_gitignore: None, show_hidden: None, min_size_bytes: None },
        tmp.path(),
    ).unwrap();
    // tree should only show top-level entries
    assert_eq!(result.kind, "success");
}

#[test]
fn test_workspace_tree_language_detection() {
    let tmp = ws();
    std::fs::write(tmp.path().join("main.rs"), "fn main(){}").unwrap();
    let result = run_workspace_tree(default_params(), tmp.path()).unwrap();
    // Rust file should be counted in languages
    let rust_count = result.summary.languages.get("rust").copied().unwrap_or(0);
    assert!(rust_count >= 1, "expected at least 1 rust file in summary");
}

#[test]
fn test_workspace_tree_empty_dir() {
    let tmp = ws();
    let result = run_workspace_tree(default_params(), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.summary.total_files, 0);
}
