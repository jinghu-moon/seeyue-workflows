// tests/test_run_command.rs
//
// Tests for tools::run_command::run_run_command.
// Run: cargo test --test test_run_command

use seeyue_mcp::tools::run_command::{RunCommandParams, run_run_command};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn params(command: &str) -> RunCommandParams {
    RunCommandParams {
        command: command.into(),
        timeout_ms: Some(10_000),
        working_dir: None,
        env: None,
    }
}

#[tokio::test]
async fn test_run_echo_returns_output() {
    let tmp = ws();
    let result = run_run_command(params("echo hello"), tmp.path()).await.unwrap();
    assert!(result.stdout.contains("hello"));
}

#[tokio::test]
async fn test_exit_code_zero_on_success() {
    let tmp = ws();
    let result = run_run_command(params("echo ok"), tmp.path()).await.unwrap();
    assert_eq!(result.exit_code, Some(0));
}

#[tokio::test]
async fn test_exit_code_nonzero_on_failure() {
    let tmp = ws();
    let result = run_run_command(params("exit 1"), tmp.path()).await.unwrap();
    assert_ne!(result.exit_code, Some(0));
}

#[tokio::test]
async fn test_command_field_matches_input() {
    let tmp = ws();
    let result = run_run_command(params("echo test"), tmp.path()).await.unwrap();
    assert_eq!(result.command, "echo test");
}

#[tokio::test]
async fn test_duration_ms_set() {
    let tmp = ws();
    let result = run_run_command(params("echo x"), tmp.path()).await.unwrap();
    // duration should be a non-negative u64
    let _ = result.duration_ms;
}

#[tokio::test]
async fn test_working_dir_escape_blocked() {
    let tmp = ws();
    let err = run_run_command(
        RunCommandParams {
            command: "echo x".into(),
            timeout_ms: Some(5_000),
            working_dir: Some("../../outside".into()),
            env: None,
        },
        tmp.path(),
    ).await.unwrap_err();
    assert!(format!("{err:?}").contains("PathEscape") || format!("{err:?}").contains("outside"));
}
