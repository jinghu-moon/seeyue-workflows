// tests/test_search_workspace.rs
//
// Tests for tools::search_workspace::run_search_workspace.
// Run: cargo test --test test_search_workspace

use seeyue_mcp::tools::search_workspace::{SearchWorkspaceParams, run_search_workspace};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn params(pattern: &str) -> SearchWorkspaceParams {
    SearchWorkspaceParams {
        pattern: pattern.into(),
        is_regex: None,
        file_glob: None,
        context_lines: None,
        max_results: None,
    }
}

#[test]
fn test_search_finds_match() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "hello world\n").unwrap();
    let result = run_search_workspace(params("hello"), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
    assert!(result.total_matches >= 1);
}

#[test]
fn test_search_no_match_returns_empty() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "hello world\n").unwrap();
    let result = run_search_workspace(params("zzznomatch"), tmp.path()).unwrap();
    assert_eq!(result.total_matches, 0);
    assert!(result.matches.is_empty());
}

#[test]
fn test_search_regex_pattern() {
    let tmp = ws();
    std::fs::write(tmp.path().join("b.txt"), "foo123 bar456\n").unwrap();
    let result = run_search_workspace(
        SearchWorkspaceParams {
            pattern: r"\d+".into(),
            is_regex: Some(true),
            file_glob: None,
            context_lines: None,
            max_results: None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.total_matches >= 1);
}

#[test]
fn test_search_empty_pattern_errors() {
    let tmp = ws();
    let err = run_search_workspace(params(""), tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("MissingParameter") || msg.contains("pattern"));
}

#[test]
fn test_search_match_has_path_and_line() {
    let tmp = ws();
    std::fs::write(tmp.path().join("c.txt"), "target line\n").unwrap();
    let result = run_search_workspace(params("target"), tmp.path()).unwrap();
    let m = &result.matches[0];
    assert!(!m.path.is_empty());
    assert!(m.line >= 1);
}

#[test]
fn test_search_file_glob_filter() {
    let tmp = ws();
    std::fs::write(tmp.path().join("main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(tmp.path().join("main.txt"), "fn main() {}\n").unwrap();
    let result = run_search_workspace(
        SearchWorkspaceParams {
            pattern: "main".into(),
            is_regex: None,
            file_glob: Some("*.rs".into()),
            context_lines: None,
            max_results: None,
        },
        tmp.path(),
    ).unwrap();
    // All matches should be from .rs files
    for m in &result.matches {
        assert!(m.path.ends_with(".rs"), "expected .rs file, got {}", m.path);
    }
}
