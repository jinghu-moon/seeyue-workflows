// tests/test_hook_binary.rs
//
// P2 Integration tests for the `sy-hook` binary.
// Spawns the binary as a child process, injects JSON via stdin,
// and asserts on stdout JSON + exit code.
//
// Run:
//   cargo test --test test_hook_binary -- --nocapture
//
// These tests use the debug binary by default. For production timing:
//   cargo test --release --test test_hook_binary -- --nocapture

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use serde_json::Value;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn hook_binary() -> PathBuf {
    // cargo test builds to target/debug or target/release
    let mut path = PathBuf::from(env!("CARGO_BIN_EXE_sy-hook"));
    // Fallback: find it relative to the manifest dir
    if !path.exists() {
        path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join("sy-hook.exe");
    }
    assert!(path.exists(), "sy-hook binary not found at {:?}", path);
    path
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run sy-hook with the given JSON input and return (stdout_json, exit_code).
fn run_hook(input_json: &str) -> (Value, i32) {
    let start = Instant::now();

    let mut child = Command::new(hook_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(project_root())
        .spawn()
        .expect("Failed to spawn sy-hook");

    // Write stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .expect("Failed to write stdin");
    }

    let output = child.wait_with_output().expect("Failed to wait for sy-hook");
    let elapsed = start.elapsed();

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Debug output for --nocapture
    println!(
        "  [{:.1}ms] exit={} stdout={}",
        elapsed.as_secs_f64() * 1000.0,
        exit_code,
        if stdout.len() > 200 {
            format!("{}...", &stdout[..200])
        } else {
            stdout.clone()
        }
    );

    let json: Value = if stdout.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&stdout).unwrap_or(Value::Null)
    };

    (json, exit_code)
}

fn assert_verdict(json: &Value, expected: &str) {
    let actual = json
        .get("verdict")
        .and_then(|v| v.as_str())
        .unwrap_or("(missing)");
    assert_eq!(actual, expected, "Expected verdict '{}', got '{}'", expected, actual);
}

// ─── PreToolUse:Bash ────────────────────────────────────────────────────────

#[test]
fn test_pretool_bash_safe_command() {
    println!("\n=== PreToolUse:Bash — safe command (ls) ===");
    let (json, code) = run_hook(r#"{"hook_event":"PreToolUse:Bash","tool_input":{"command":"ls"}}"#);
    assert_eq!(code, 0, "Safe command should exit 0");
    assert_verdict(&json, "allow");
}

#[test]
fn test_pretool_bash_destructive() {
    println!("\n=== PreToolUse:Bash — destructive (rm -rf /) ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PreToolUse:Bash","tool_input":{"command":"rm -rf /"}}"#,
    );
    assert_eq!(code, 2, "Destructive command should exit 2");
    let verdict = json.get("verdict").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        verdict == "block" || verdict == "block_with_approval_request",
        "Destructive command should block, got: {}",
        verdict
    );
}

#[test]
fn test_pretool_bash_git_push() {
    println!("\n=== PreToolUse:Bash — git push ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PreToolUse:Bash","tool_input":{"command":"git push"}}"#,
    );
    assert_eq!(code, 2, "git push should exit 2");
    let verdict = json.get("verdict").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        verdict == "block" || verdict == "block_with_approval_request",
        "git push should block, got: {}",
        verdict
    );
}

#[test]
fn test_pretool_bash_verify_command() {
    println!("\n=== PreToolUse:Bash — verify command (cargo test) ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PreToolUse:Bash","tool_input":{"command":"cargo test"}}"#,
    );
    assert_eq!(code, 0, "Verify command should exit 0");
    assert_verdict(&json, "allow");
}

// ─── PreToolUse:Write ───────────────────────────────────────────────────────

#[test]
fn test_pretool_write_secret_env() {
    println!("\n=== PreToolUse:Write — .env (secret material) ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PreToolUse:Write","tool_input":{"file_path":".env"}}"#,
    );
    assert_eq!(code, 2, ".env write should exit 2");
    assert_verdict(&json, "block");
}

#[test]
fn test_pretool_write_normal_source() {
    println!("\n=== PreToolUse:Write — src/main.rs (workspace file) ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PreToolUse:Write","tool_input":{"file_path":"src/main.rs"}}"#,
    );
    assert_eq!(code, 0, "Normal source write should exit 0");
    assert_verdict(&json, "allow");
}

#[test]
fn test_pretool_write_pem_file() {
    println!("\n=== PreToolUse:Write — server.pem (secret material) ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PreToolUse:Write","tool_input":{"file_path":"certs/server.pem"}}"#,
    );
    assert_eq!(code, 2, "*.pem write should exit 2");
    assert_verdict(&json, "block");
}

// ─── Stop ───────────────────────────────────────────────────────────────────

#[test]
fn test_stop_clean_session() {
    println!("\n=== Stop — clean session ===");
    let (json, code) = run_hook(r#"{"hook_event":"Stop"}"#);
    assert_eq!(code, 0, "Stop on clean session should exit 0");
    assert_verdict(&json, "allow");
}

// ─── SessionStart ───────────────────────────────────────────────────────────

#[test]
fn test_session_start_bootstrap() {
    println!("\n=== SessionStart — bootstrap injection ===");
    let (json, code) = run_hook(r#"{"hook_event":"SessionStart"}"#);
    assert_eq!(code, 0, "SessionStart should exit 0");
    assert_verdict(&json, "allow");

    // Verify bootstrap context is present
    let ctx = json
        .get("additional_context")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        ctx.contains("<SY-BOOTSTRAP>"),
        "SessionStart should include <SY-BOOTSTRAP> in additional_context"
    );
    assert!(
        ctx.contains("sy-workflow"),
        "Bootstrap should reference sy-workflow"
    );
    assert!(
        ctx.contains("sy-constraints"),
        "Bootstrap should reference sy-constraints"
    );

    // Verify hookSpecificOutput
    let hook_output = json.get("hookSpecificOutput");
    assert!(hook_output.is_some(), "Should have hookSpecificOutput");
    assert_eq!(
        hook_output
            .unwrap()
            .get("hookEventName")
            .and_then(|v| v.as_str()),
        Some("SessionStart")
    );
}

#[test]
fn test_session_start_git_context() {
    println!("\n=== SessionStart — git context ===");
    let (json, _) = run_hook(r#"{"hook_event":"SessionStart"}"#);
    let ctx = json
        .get("additional_context")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        ctx.contains("GIT: branch="),
        "SessionStart should include git branch info"
    );
}

// ─── UserPromptSubmit ───────────────────────────────────────────────────────

#[test]
fn test_prompt_submit_no_workflow() {
    println!("\n=== UserPromptSubmit — no active workflow ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"UserPromptSubmit","prompt":"implement the feature"}"#,
    );
    assert_eq!(code, 0);
    assert_verdict(&json, "allow");
    // Without active workflow session, should be inactive
    let reason = json.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        reason.contains("inactive") || reason.contains("no_match"),
        "Should indicate inactive/no_match without workflow, got: {}",
        reason
    );
}

// ─── PostToolUse:Write ──────────────────────────────────────────────────────

#[test]
fn test_posttool_write_allow() {
    println!("\n=== PostToolUse:Write — always allow ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PostToolUse:Write","tool_input":{"file_path":"src/app.rs"},"tool_name":"Write"}"#,
    );
    assert_eq!(code, 0, "PostToolUse:Write should always exit 0");
    assert_verdict(&json, "allow");
}

// ─── PostToolUse:Bash ───────────────────────────────────────────────────────

#[test]
fn test_posttool_bash_cargo_test() {
    println!("\n=== PostToolUse:Bash — cargo test (verification capture) ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PostToolUse:Bash","tool_input":{"command":"cargo test"},"tool_response":{"stdout":"test result: ok. 5 passed","returncode":0}}"#,
    );
    assert_eq!(code, 0, "PostToolUse:Bash should always exit 0");
    assert_verdict(&json, "allow");
}

#[test]
fn test_posttool_bash_non_verification() {
    println!("\n=== PostToolUse:Bash — non-verification command ===");
    let (json, code) = run_hook(
        r#"{"hook_event":"PostToolUse:Bash","tool_input":{"command":"echo hello"},"tool_response":{"stdout":"hello","returncode":0}}"#,
    );
    assert_eq!(code, 0);
    assert_verdict(&json, "allow");
    let reason = json.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        reason.contains("non_verification") || reason.contains("allow"),
        "Non-verification command should indicate so, got: {}",
        reason
    );
}

// ─── Fail-open / Robustness ────────────────────────────────────────────────

#[test]
fn test_failopen_malformed_json() {
    println!("\n=== Fail-open — malformed JSON ===");
    let (json, code) = run_hook("{{{invalid json");
    assert_eq!(code, 0, "Malformed JSON should fail-open with exit 0");
    assert_verdict(&json, "allow");
}

#[test]
fn test_failopen_empty_stdin() {
    println!("\n=== Fail-open — empty stdin ===");
    let (json, code) = run_hook("");
    assert_eq!(code, 0, "Empty stdin should fail-open with exit 0");
    assert_verdict(&json, "allow");
}

#[test]
fn test_failopen_empty_json() {
    println!("\n=== Fail-open — empty JSON object ===");
    let (json, code) = run_hook("{}");
    assert_eq!(code, 0, "Empty JSON should fail-open with exit 0");
    assert_verdict(&json, "allow");
}

// ─── DRY_RUN mode ───────────────────────────────────────────────────────────

#[test]
fn test_dry_run_always_allows() {
    println!("\n=== DRY_RUN — destructive command should still allow ===");

    let mut child = Command::new(hook_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(project_root())
        .env("SY_HOOK_DRY_RUN", "1")
        .spawn()
        .expect("Failed to spawn sy-hook");

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(br#"{"hook_event":"PreToolUse:Bash","tool_input":{"command":"rm -rf /"}}"#)
            .expect("Failed to write stdin");
    }

    let output = child.wait_with_output().expect("Failed to wait");
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);

    println!("  exit={} stdout={}", exit_code, stdout);

    assert_eq!(exit_code, 0, "DRY_RUN should always exit 0");
    assert_verdict(&json, "allow");
    let reason = json.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        reason.contains("dry_run"),
        "DRY_RUN reason should mention dry_run, got: {}",
        reason
    );
}

// ─── Event inference ────────────────────────────────────────────────────────

#[test]
fn test_infer_event_from_tool_name() {
    println!("\n=== Infer event from tool_name (no hook_event) ===");
    // Without hook_event, should infer PreToolUse:Bash from tool_name + no tool_response
    let (json, code) = run_hook(
        r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#,
    );
    assert_eq!(code, 0);
    assert_verdict(&json, "allow");
}

// ─── Performance gate ───────────────────────────────────────────────────────

#[test]
fn test_cold_start_under_threshold() {
    println!("\n=== Performance — cold start latency ===");

    // Run 7 sequential invocations and measure timing.
    // When tests run in parallel, early runs may be inflated by CPU contention
    // from other test processes also spawning sy-hook.exe concurrently.
    let mut durations = Vec::new();
    for i in 0..7 {
        let start = Instant::now();
        let (_, code) = run_hook(
            r#"{"hook_event":"PreToolUse:Bash","tool_input":{"command":"ls"}}"#,
        );
        let elapsed = start.elapsed();
        durations.push(elapsed);
        assert_eq!(code, 0);
        println!("  Run {}: {:.1}ms", i + 1, elapsed.as_secs_f64() * 1000.0);
    }

    // Skip first 2 runs (OS cold cache + parallel test contention), check last 5.
    // Use median (index 2 of sorted 5) — robust against single outliers.
    let mut warm: Vec<_> = durations[2..].to_vec();
    warm.sort();
    let median_warm = warm[warm.len() / 2];

    // Threshold: 500ms debug, 100ms release (includes process spawn overhead)
    let limit_ms = if cfg!(debug_assertions) { 500 } else { 100 };
    println!(
        "  Median warm: {:.1}ms (limit: {}ms, {})",
        median_warm.as_secs_f64() * 1000.0,
        limit_ms,
        if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" }
    );

    assert!(
        median_warm.as_millis() < limit_ms,
        "Median warm start {:.1}ms exceeds {}ms limit",
        median_warm.as_secs_f64() * 1000.0,
        limit_ms
    );
}
