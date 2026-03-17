// tests/test_read_range.rs
//
// Tests for tools::read_range::run_read_range.
// Run: cargo test --test test_read_range

use seeyue_mcp::tools::read_range::{ReadRangeParams, run_read_range};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

const SRC: &str = "line1\nline2\nline3\nline4\nline5\n";

#[test]
fn test_read_range_basic() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), SRC).unwrap();
    let result = run_read_range(
        ReadRangeParams { path: "f.txt".into(), start: Some(2), end: Some(4), symbol: None, context_lines: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.start, 2);
    assert_eq!(result.end, 4);
    assert!(result.content.contains("line2"));
    assert!(result.content.contains("line4"));
    assert!(!result.content.contains("line1"));
}

#[test]
fn test_read_range_full_file() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), SRC).unwrap();
    let result = run_read_range(
        ReadRangeParams { path: "f.txt".into(), start: Some(1), end: Some(5), symbol: None, context_lines: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.total_lines, 5);
    assert!(result.content.contains("line1"));
    assert!(result.content.contains("line5"));
}

#[test]
fn test_read_range_with_context() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), SRC).unwrap();
    let result = run_read_range(
        ReadRangeParams { path: "f.txt".into(), start: Some(3), end: Some(3), symbol: None, context_lines: Some(1) },
        tmp.path(),
    ).unwrap();
    // context=1 expands to lines 2-4
    assert!(result.content.contains("line2"));
    assert!(result.content.contains("line3"));
    assert!(result.content.contains("line4"));
}

#[test]
fn test_read_range_nonexistent_file() {
    let tmp = ws();
    let err = run_read_range(
        ReadRangeParams { path: "no.txt".into(), start: Some(1), end: Some(1), symbol: None, context_lines: None },
        tmp.path(),
    ).unwrap_err();
    assert!(format!("{err:?}").contains("FileNotFound") || format!("{err:?}").contains("not found"));
}

#[test]
fn test_read_range_missing_start_and_symbol_errors() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), SRC).unwrap();
    let err = run_read_range(
        ReadRangeParams { path: "f.txt".into(), start: None, end: None, symbol: None, context_lines: None },
        tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("MissingParameter") || msg.contains("start") || msg.contains("symbol"));
}

#[test]
fn test_read_range_by_symbol() {
    let tmp = ws();
    let src = "fn foo() {\n    let x = 1;\n}\nfn bar() {\n    let y = 2;\n}\n";
    std::fs::write(tmp.path().join("s.rs"), src).unwrap();
    let result = run_read_range(
        ReadRangeParams { path: "s.rs".into(), start: None, end: None, symbol: Some("foo".into()), context_lines: None },
        tmp.path(),
    ).unwrap();
    assert!(result.symbol_start.is_some());
    assert!(result.content.contains("foo"));
}
