// tests/test_format_file.rs
//
// Tests for tools::format_file::run_format_file.
// Run: cargo test --test test_format_file

use seeyue_mcp::tools::format_file::{FormatFileParams, run_format_file};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn params(path: &str, check_only: bool) -> FormatFileParams {
    FormatFileParams {
        path:       path.into(),
        check_only: Some(check_only),
    }
}

#[test]
fn test_format_file_not_found_errors() {
    let tmp = ws();
    let err = run_format_file(params("does_not_exist.rs", false), tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("FileNotFound") || msg.contains("NotFound"), "unexpected: {msg}");
}

#[test]
fn test_format_file_path_escape_blocked() {
    let tmp = ws();
    let err = run_format_file(params("../../outside.rs", false), tmp.path()).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("PathEscape") || msg.contains("outside"), "unexpected: {msg}");
}

#[test]
fn test_format_file_unsupported_language() {
    let tmp = ws();
    std::fs::write(tmp.path().join("foo.xyz"), "data").unwrap();
    let result = run_format_file(params("foo.xyz", false), tmp.path()).unwrap();
    assert_eq!(result.status, "UNSUPPORTED");
    assert_eq!(result.kind, "success");
}

#[test]
fn test_format_rust_file_check_only() {
    let tmp = ws();
    // Valid but unformatted Rust — rustfmt may or may not be available
    std::fs::write(tmp.path().join("main.rs"), "fn main(){println!(\"hi\");}").unwrap();
    let result = run_format_file(params("main.rs", true), tmp.path()).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.check_only, true);
    assert!(
        result.status == "already_formatted"
            || result.status == "needs_formatting"
            || result.status == "FORMATTER_NOT_FOUND",
        "unexpected status: {}", result.status
    );
}

#[test]
fn test_format_file_formatter_field_populated() {
    let tmp = ws();
    std::fs::write(tmp.path().join("main.rs"), "fn main(){}").unwrap();
    let result = run_format_file(params("main.rs", true), tmp.path()).unwrap();
    assert!(!result.formatter.is_empty());
}

#[test]
fn test_format_file_duration_accessible() {
    let tmp = ws();
    std::fs::write(tmp.path().join("foo.txt"), "data").unwrap();
    let result = run_format_file(params("foo.txt", false), tmp.path()).unwrap();
    let _ = result.duration_ms;
}
