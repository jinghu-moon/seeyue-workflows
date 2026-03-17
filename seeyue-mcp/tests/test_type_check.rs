// tests/test_type_check.rs
//
// Tests for tools::type_check::run_type_check.
// Run: cargo test --test test_type_check

use seeyue_mcp::tools::type_check::{TypeCheckParams, run_type_check};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[test]
fn test_nonexistent_path_errors() {
    let tmp = ws();
    let err = run_type_check(
        TypeCheckParams { path: "ghost.ts".into(), language: None },
        tmp.path(),
    ).unwrap_err();
    assert!(format!("{err:?}").contains("FileNotFound") || format!("{err:?}").contains("not found"));
}

#[test]
fn test_unsupported_language_errors() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "hello").unwrap();
    let err = run_type_check(
        TypeCheckParams { path: "f.txt".into(), language: Some("cobol".into()) },
        tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("UnsupportedLanguage") || msg.contains("cobol"));
}

#[test]
fn test_typescript_tool_not_found_or_ok() {
    let tmp = ws();
    std::fs::write(tmp.path().join("x.ts"), "const x: number = 42;\n").unwrap();
    let result = run_type_check(
        TypeCheckParams { path: "x.ts".into(), language: None },
        tmp.path(),
    ).unwrap();
    // tsc may or may not be installed in CI — both outcomes are valid
    assert!(result.status == "ok" || result.status == "errors" || result.status == "TOOL_NOT_FOUND",
        "unexpected status: {}", result.status);
}

#[test]
fn test_python_tool_not_found_or_ok() {
    let tmp = ws();
    std::fs::write(tmp.path().join("m.py"), "x: int = 42\n").unwrap();
    let result = run_type_check(
        TypeCheckParams { path: "m.py".into(), language: None },
        tmp.path(),
    ).unwrap();
    assert!(result.status == "ok" || result.status == "errors" || result.status == "TOOL_NOT_FOUND",
        "unexpected status: {}", result.status);
}

#[test]
fn test_language_field_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("y.ts"), "const y = 1;\n").unwrap();
    let result = run_type_check(
        TypeCheckParams { path: "y.ts".into(), language: None },
        tmp.path(),
    ).unwrap();
    assert!(!result.language.is_empty());
}

#[test]
fn test_tool_field_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("z.ts"), "const z = 1;\n").unwrap();
    let result = run_type_check(
        TypeCheckParams { path: "z.ts".into(), language: None },
        tmp.path(),
    ).unwrap();
    assert!(!result.tool.is_empty());
}
