// tests/test_read_compressed.rs
//
// Tests for tools::read_compressed::run_read_compressed.
// Run: cargo test --test test_read_compressed

use seeyue_mcp::tools::read_compressed::{ReadCompressedParams, run_read_compressed};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[test]
fn test_small_file_no_compression() {
    let tmp = ws();
    std::fs::write(tmp.path().join("small.txt"), "hello world\n").unwrap();
    let result = run_read_compressed(
        ReadCompressedParams { path: "small.txt".into(), token_budget: Some(1000) },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.compression_level, 0);
    assert!(result.content.contains("hello"));
}

#[test]
fn test_large_file_triggers_compression() {
    let tmp = ws();
    // Create a file larger than default 800-token budget (~3200 chars)
    let big = "x ".repeat(2000) + "\n";
    std::fs::write(tmp.path().join("big.txt"), &big).unwrap();
    let result = run_read_compressed(
        ReadCompressedParams { path: "big.txt".into(), token_budget: Some(100) },
        tmp.path(),
    ).unwrap();
    assert!(result.compression_level > 0, "large file should trigger compression");
    assert!(result.token_estimate <= result.token_budget + result.token_budget / 2,
        "token estimate {} should be near budget {}", result.token_estimate, result.token_budget);
}

#[test]
fn test_token_budget_field_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "hi\n").unwrap();
    let result = run_read_compressed(
        ReadCompressedParams { path: "f.txt".into(), token_budget: Some(500) },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.token_budget, 500);
}

#[test]
fn test_default_token_budget() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "hello\n").unwrap();
    let result = run_read_compressed(
        ReadCompressedParams { path: "f.txt".into(), token_budget: None },
        tmp.path(),
    ).unwrap();
    // Default is 800
    assert_eq!(result.token_budget, 800);
}

#[test]
fn test_nonexistent_file_errors() {
    let tmp = ws();
    let err = run_read_compressed(
        ReadCompressedParams { path: "ghost.txt".into(), token_budget: None },
        tmp.path(),
    ).unwrap_err();
    assert!(format!("{err:?}").contains("FileNotFound") || format!("{err:?}").contains("not found"));
}

#[test]
fn test_path_field_matches_input() {
    let tmp = ws();
    std::fs::write(tmp.path().join("named.txt"), "content\n").unwrap();
    let result = run_read_compressed(
        ReadCompressedParams { path: "named.txt".into(), token_budget: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.path, "named.txt");
}
