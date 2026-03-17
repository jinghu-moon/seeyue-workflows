// tests/test_resolve_path.rs
//
// Tests for tools::resolve_path::run_resolve_path.
// Run: cargo test --test test_resolve_path

use seeyue_mcp::tools::resolve_path::{ResolvePathParams, run_resolve_path};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[test]
fn test_resolve_existing_file() {
    let tmp = ws();
    let file = tmp.path().join("foo.txt");
    std::fs::write(&file, b"hi").unwrap();

    let result = run_resolve_path(
        ResolvePathParams { path: "foo.txt".into() },
        tmp.path(),
    ).unwrap();

    assert_eq!(result.kind, "success");
    assert!(result.exists);
    assert!(!result.is_dir);
    assert!(result.in_workspace);
    assert_eq!(result.input, "foo.txt");
}

#[test]
fn test_resolve_nonexistent_file() {
    let tmp = ws();
    let result = run_resolve_path(
        ResolvePathParams { path: "ghost.txt".into() },
        tmp.path(),
    ).unwrap();
    assert!(!result.exists);
    assert!(result.in_workspace);
}

#[test]
fn test_resolve_directory() {
    let tmp = ws();
    std::fs::create_dir(tmp.path().join("subdir")).unwrap();
    let result = run_resolve_path(
        ResolvePathParams { path: "subdir".into() },
        tmp.path(),
    ).unwrap();
    assert!(result.exists);
    assert!(result.is_dir);
}

#[test]
fn test_resolve_nested_path() {
    let tmp = ws();
    std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();
    std::fs::write(tmp.path().join("a/b/c.rs"), b"").unwrap();
    let result = run_resolve_path(
        ResolvePathParams { path: "a/b/c.rs".into() },
        tmp.path(),
    ).unwrap();
    assert!(result.exists);
    assert!(!result.is_dir);
}

#[test]
fn test_resolve_path_escape_blocked() {
    let tmp = ws();
    let result = run_resolve_path(
        ResolvePathParams { path: "../../etc/passwd".into() },
        tmp.path(),
    );
    assert!(result.is_err(), "path escape should be blocked");
}

#[test]
fn test_relative_field_is_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("hello.txt"), b"").unwrap();
    let result = run_resolve_path(
        ResolvePathParams { path: "hello.txt".into() },
        tmp.path(),
    ).unwrap();
    assert!(!result.relative.is_empty());
    assert!(!result.absolute.is_empty());
}
