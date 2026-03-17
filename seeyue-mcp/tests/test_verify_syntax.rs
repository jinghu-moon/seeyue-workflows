// tests/test_verify_syntax.rs
//
// Tests for tools::verify_syntax::run_verify_syntax.
// Run: cargo test --test test_verify_syntax

use seeyue_mcp::tools::verify_syntax::{VerifySyntaxParams, run_verify_syntax};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn params_content(content: &str, language: &str) -> VerifySyntaxParams {
    VerifySyntaxParams {
        path: None,
        content: Some(content.to_string()),
        language: Some(language.to_string()),
    }
}

// ─── content-based ───────────────────────────────────────────────────────────

#[test]
fn test_valid_rust_content() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("fn main() { let x = 1; }", "rust"),
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert!(result.valid);
    assert_eq!(result.language, "rust");
}

#[test]
fn test_invalid_rust_content() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("fn main( { let x = 1; ", "rust"),
        tmp.path(),
    ).unwrap();
    assert!(!result.valid, "broken Rust should not be valid");
    assert!(result.errors.is_some());
}

#[test]
fn test_valid_python_content() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("def hello():\n    return 42\n", "python"),
        tmp.path(),
    ).unwrap();
    assert!(result.valid);
    assert_eq!(result.language, "python");
}

#[test]
fn test_valid_typescript_content() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("const x: number = 42;", "typescript"),
        tmp.path(),
    ).unwrap();
    assert!(result.valid);
}

#[test]
fn test_empty_content() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("", "rust"),
        tmp.path(),
    ).unwrap();
    // Empty source is syntactically valid (no tokens = no errors)
    assert_eq!(result.kind, "success");
}

#[test]
fn test_unknown_language_returns_note() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("some text", "cobol"),
        tmp.path(),
    ).unwrap();
    // Unknown language: tool should not panic; note should indicate unsupported
    assert!(result.note.is_some() || result.valid,
        "unknown language should either set a note or return valid=true");
}

// ─── file-based ───────────────────────────────────────────────────────────────

#[test]
fn test_verify_syntax_from_file() {
    let tmp = ws();
    let file = tmp.path().join("main.rs");
    std::fs::write(&file, b"fn greet(name: &str) -> String { format!(\"Hello {name}\") }").unwrap();
    let result = run_verify_syntax(
        VerifySyntaxParams { path: Some("main.rs".into()), content: None, language: None },
        tmp.path(),
    ).unwrap();
    assert!(result.valid);
    assert_eq!(result.language, "rust");
    assert!(result.path.is_some());
}

#[test]
fn test_verify_syntax_file_not_found() {
    let tmp = ws();
    let result = run_verify_syntax(
        VerifySyntaxParams { path: Some("missing.rs".into()), content: None, language: None },
        tmp.path(),
    );
    assert!(result.is_err());
}

#[test]
fn test_parse_ms_is_set() {
    let tmp = ws();
    let result = run_verify_syntax(
        params_content("fn f() {}", "rust"),
        tmp.path(),
    ).unwrap();
    // parse_ms should be a non-negative duration (u128)
    let _ = result.parse_ms; // just ensure the field exists and compiles
}
