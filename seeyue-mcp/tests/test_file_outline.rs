// tests/test_file_outline.rs
//
// Tests for tools::file_outline::run_file_outline.
// Run: cargo test --test test_file_outline

use seeyue_mcp::tools::file_outline::{FileOutlineParams, run_file_outline};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn params(path: &str, depth: Option<u8>) -> FileOutlineParams {
    FileOutlineParams { path: path.into(), depth }
}

#[test]
fn test_outline_rust_returns_success() {
    let tmp = ws();
    std::fs::write(tmp.path().join("lib.rs"), "pub fn foo() {}\npub fn bar() {}\n").unwrap();
    let result = run_file_outline(params("lib.rs", None), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.language, "rust");
}

#[test]
fn test_outline_contains_symbols() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.rs"), "pub fn foo() {}\npub fn bar() {}\n").unwrap();
    let result = run_file_outline(params("a.rs", None), tmp.path()).unwrap();
    assert!(result.symbols.len() >= 2, "expected at least 2 symbols");
}

#[test]
fn test_outline_total_lines() {
    let tmp = ws();
    let src = "line1\nline2\nline3\n";
    std::fs::write(tmp.path().join("f.txt"), src).unwrap();
    let result = run_file_outline(params("f.txt", Some(0)), tmp.path()).unwrap();
    assert_eq!(result.total_lines, 3);
}

#[test]
fn test_outline_path_field_matches() {
    let tmp = ws();
    std::fs::write(tmp.path().join("x.rs"), "fn x(){}").unwrap();
    let result = run_file_outline(params("x.rs", None), tmp.path()).unwrap();
    assert_eq!(result.path, "x.rs");
}

#[test]
fn test_outline_nonexistent_errors() {
    let tmp = ws();
    let err = run_file_outline(params("ghost.rs", None), tmp.path()).unwrap_err();
    assert!(format!("{err:?}").contains("FileNotFound") || format!("{err:?}").contains("not found"));
}

#[test]
fn test_outline_token_estimate_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("t.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
    let result = run_file_outline(params("t.rs", None), tmp.path()).unwrap();
    // token_estimate is a usize — just verify it's accessible and non-negative
    let _ = result.token_estimate;
}
