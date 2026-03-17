// tests/test_preview_edit.rs
//
// Tests for tools::preview_edit::run_preview_edit.
// Run: cargo test --test test_preview_edit

use seeyue_mcp::tools::preview_edit::{PreviewEditParams, run_preview_edit};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn make_file(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

#[test]
fn test_preview_shows_diff() {
    let tmp = ws();
    make_file(tmp.path(), "foo.txt", "hello world");
    let result = run_preview_edit(
        PreviewEditParams {
            file_path:  "foo.txt".into(),
            old_string: "world".into(),
            new_string: "rust".into(),
            replace_all: None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.would_apply);
    assert_eq!(result.replacements, 1);
}

#[test]
fn test_preview_not_found_errors() {
    let tmp = ws();
    let err = run_preview_edit(
        PreviewEditParams {
            file_path:  "missing.txt".into(),
            old_string: "x".into(),
            new_string: "y".into(),
            replace_all: None,
        },
        tmp.path(),
    ).unwrap_err();
    assert!(format!("{err:?}").contains("FileNotFound") || format!("{err:?}").contains("missing"));
}

#[test]
fn test_preview_old_not_found_errors() {
    let tmp = ws();
    make_file(tmp.path(), "f.txt", "abc");
    let err = run_preview_edit(
        PreviewEditParams {
            file_path:  "f.txt".into(),
            old_string: "zzz".into(),
            new_string: "y".into(),
            replace_all: None,
        },
        tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("NotFound") || msg.contains("not found") || msg.contains("NoMatch"));
}

#[test]
fn test_preview_replace_all() {
    let tmp = ws();
    make_file(tmp.path(), "g.txt", "aa bb aa");
    let result = run_preview_edit(
        PreviewEditParams {
            file_path:  "g.txt".into(),
            old_string: "aa".into(),
            new_string: "cc".into(),
            replace_all: Some(true),
        },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.replacements, 2);
}

#[test]
fn test_preview_does_not_modify_file() {
    let tmp = ws();
    make_file(tmp.path(), "h.txt", "original");
    let _ = run_preview_edit(
        PreviewEditParams {
            file_path:  "h.txt".into(),
            old_string: "original".into(),
            new_string: "changed".into(),
            replace_all: None,
        },
        tmp.path(),
    ).unwrap();
    // File must remain unchanged
    let after = std::fs::read_to_string(tmp.path().join("h.txt")).unwrap();
    assert_eq!(after, "original", "preview must not write to disk");
}

#[test]
fn test_preview_syntax_valid_after_for_rust() {
    let tmp = ws();
    make_file(tmp.path(), "main.rs", "fn main() { let x = 1; }");
    let result = run_preview_edit(
        PreviewEditParams {
            file_path:  "main.rs".into(),
            old_string: "let x = 1;".into(),
            new_string: "let x = 2;".into(),
            replace_all: None,
        },
        tmp.path(),
    ).unwrap();
    assert!(result.syntax_valid_after, "valid Rust after edit should pass syntax check");
}
