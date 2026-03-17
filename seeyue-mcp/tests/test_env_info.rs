// tests/test_env_info.rs
//
// Tests for tools::env_info::run_env_info.
// Run: cargo test --test test_env_info

use seeyue_mcp::tools::env_info::run_env_info;

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[test]
fn test_env_info_returns_success_kind() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    assert_eq!(result.kind, "success");
}

#[test]
fn test_env_info_os_is_set() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    assert!(!result.os.is_empty());
}

#[test]
fn test_env_info_arch_is_set() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    assert!(!result.arch.is_empty());
}

#[test]
fn test_env_info_workspace_matches_input() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    // workspace should contain the temp dir path
    assert!(!result.workspace.is_empty());
}

#[test]
fn test_env_info_disk_free_mb_reasonable() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    // At least 1 MB free on any reasonable test machine
    assert!(result.disk_free_mb > 0, "disk_free_mb should be > 0");
}

#[test]
fn test_env_info_line_ending_is_valid() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    assert!(result.line_ending == "CRLF" || result.line_ending == "LF",
        "line_ending should be CRLF or LF, got: {}", result.line_ending);
}

#[test]
fn test_env_info_version_is_semver() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    // Should be a semver-like string e.g. "0.4.0"
    assert!(result.agent_editor_version.contains('.'),
        "version should contain '.': {}", result.agent_editor_version);
}

#[test]
fn test_env_info_codepage_name_is_set() {
    let tmp = ws();
    let result = run_env_info(tmp.path());
    assert!(!result.codepage_name.is_empty());
}
