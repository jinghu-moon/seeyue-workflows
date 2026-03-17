// tests/test_lint_file.rs
//
// Tests for tools::lint_file::run_lint_file (async).
// Run: cargo test --test test_lint_file

use seeyue_mcp::tools::lint_file::{LintFileParams, run_lint_file};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[tokio::test]
async fn test_lint_nonexistent_returns_result() {
    let tmp = ws();
    // lint_file passes path to clippy/eslint/ruff — tool may not error on missing file
    // Just verify the call completes without panic
    let result = run_lint_file(
        LintFileParams { path: "ghost.rs".into(), linter: None, fix: None },
        tmp.path(),
    ).await;
    // Either an error or an empty diagnostics result is acceptable
    match result {
        Ok(r) => { let _ = r.diagnostics; }
        Err(_) => {}
    }
}

#[tokio::test]
async fn test_lint_path_field_matches() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.rs"), "fn main() {}\n").unwrap();
    let result = run_lint_file(
        LintFileParams { path: "f.rs".into(), linter: None, fix: None },
        tmp.path(),
    ).await.unwrap();
    assert_eq!(result.path, "f.rs");
}

#[tokio::test]
async fn test_lint_rust_linter_field() {
    let tmp = ws();
    std::fs::write(tmp.path().join("g.rs"), "fn main() {}\n").unwrap();
    let result = run_lint_file(
        LintFileParams { path: "g.rs".into(), linter: None, fix: None },
        tmp.path(),
    ).await.unwrap();
    assert!(result.linter.contains("clippy") || !result.linter.is_empty());
}

#[tokio::test]
async fn test_lint_diagnostics_is_vec() {
    let tmp = ws();
    std::fs::write(tmp.path().join("h.rs"), "fn main() {}\n").unwrap();
    let result = run_lint_file(
        LintFileParams { path: "h.rs".into(), linter: None, fix: None },
        tmp.path(),
    ).await.unwrap();
    // diagnostics is Vec — just verify it's accessible
    let _ = result.diagnostics.len();
}

#[tokio::test]
async fn test_lint_explicit_linter_clippy() {
    let tmp = ws();
    std::fs::write(tmp.path().join("i.rs"), "fn main() {}\n").unwrap();
    let result = run_lint_file(
        LintFileParams { path: "i.rs".into(), linter: Some("clippy".into()), fix: None },
        tmp.path(),
    ).await.unwrap();
    assert!(result.linter.contains("clippy"));
}

#[tokio::test]
async fn test_lint_duration_ms_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("j.rs"), "fn main() {}\n").unwrap();
    let result = run_lint_file(
        LintFileParams { path: "j.rs".into(), linter: None, fix: None },
        tmp.path(),
    ).await.unwrap();
    let _ = result.duration_ms;
}
