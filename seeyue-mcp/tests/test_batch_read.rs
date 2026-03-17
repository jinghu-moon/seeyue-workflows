// tests/test_batch_read.rs
//
// Tests for tools::batch_read::run_batch_read.
// Run: cargo test --test test_batch_read

use seeyue_mcp::tools::batch_read::{BatchReadParams, run_batch_read};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[test]
fn test_batch_read_single_file() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let result = run_batch_read(
        BatchReadParams { paths: vec!["a.txt".into()] },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.total, 1);
    assert_eq!(result.files[0].content, "hello");
    assert!(result.files[0].error.is_none());
}

#[test]
fn test_batch_read_multiple_files() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "bbb").unwrap();
    let result = run_batch_read(
        BatchReadParams { paths: vec!["a.txt".into(), "b.txt".into()] },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.files.len(), 2);
}

#[test]
fn test_batch_read_missing_file_has_error() {
    let tmp = ws();
    let result = run_batch_read(
        BatchReadParams { paths: vec!["does_not_exist.txt".into()] },
        tmp.path(),
    ).unwrap();
    assert!(result.files[0].error.is_some());
}

#[test]
fn test_batch_read_empty_paths_errors() {
    let tmp = ws();
    let err = run_batch_read(
        BatchReadParams { paths: vec![] },
        tmp.path(),
    ).unwrap_err();
    assert!(format!("{err:?}").contains("paths") || format!("{err:?}").contains("MissingParameter"));
}

#[test]
fn test_batch_read_path_escape_has_error() {
    let tmp = ws();
    let result = run_batch_read(
        BatchReadParams { paths: vec!["../../outside.txt".into()] },
        tmp.path(),
    ).unwrap();
    // PathEscape is captured per-file as error string
    assert!(result.files[0].error.is_some());
    let err_str = result.files[0].error.as_ref().unwrap();
    assert!(
        err_str.contains("PathEscape") || err_str.contains("outside"),
        "unexpected error: {err_str}"
    );
}

#[test]
fn test_batch_read_size_matches_content_len() {
    let tmp = ws();
    std::fs::write(tmp.path().join("c.txt"), "12345").unwrap();
    let result = run_batch_read(
        BatchReadParams { paths: vec!["c.txt".into()] },
        tmp.path(),
    ).unwrap();
    let entry = &result.files[0];
    assert_eq!(entry.size, entry.content.len());
}
