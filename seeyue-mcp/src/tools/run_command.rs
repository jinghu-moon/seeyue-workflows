// src/tools/run_command.rs
//
// Controlled shell command execution with structured output.
// Security constraints:
//   - working_dir must remain within workspace
//   - stdout/stderr truncated at TRUNCATE_CHARS
//   - timeout enforced via tokio::time::timeout

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::error::ToolError;

// ─── Constants ───────────────────────────────────────────────────────────────

const TRUNCATE_CHARS: usize = 10_000;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 300_000;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct RunCommandParams {
    pub command:     String,
    pub timeout_ms:  Option<u64>,
    pub working_dir: Option<String>,
    pub env:         Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct RunCommandResult {
    pub exit_code:   Option<i32>,
    pub stdout:      String,
    pub stderr:      String,
    pub truncated:   bool,
    pub duration_ms: u64,
    pub command:     String,
    pub working_dir: String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub async fn run_run_command(
    params: RunCommandParams,
    workspace: &Path,
) -> Result<RunCommandResult, ToolError> {
    // Resolve and validate working directory
    let work_dir = if let Some(ref wd) = params.working_dir {
        // platform::path::resolve enforces workspace boundary (rejects .. escape)
        crate::platform::path::resolve(workspace, wd).map_err(|e| {
            ToolError::PathEscape {
                file_path: wd.clone(),
                hint: format!("working_dir must be within workspace: {:?}", e),
            }
        })?
    } else {
        workspace.to_path_buf()
    };

    // Clamp timeout
    let timeout_ms = params
        .timeout_ms
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .min(MAX_TIMEOUT_MS);

    // Build command (Windows: cmd /C, Unix: sh -c)
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", &params.command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", &params.command]);
        c
    };

    cmd.current_dir(&work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Inject extra env vars
    if let Some(env) = params.env {
        for (k, v) in env {
            cmd.env(k, v);
        }
    }

    let start = Instant::now();

    let output_result = timeout(
        Duration::from_millis(timeout_ms),
        cmd.output(),
    )
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match output_result {
        Err(_) => {
            // Timeout
            Ok(RunCommandResult {
                exit_code:   None,
                stdout:      String::new(),
                stderr:      format!("Command timed out after {}ms", timeout_ms),
                truncated:   false,
                duration_ms,
                command:     params.command,
                working_dir: work_dir.display().to_string(),
            })
        }
        Ok(Err(e)) => Err(ToolError::IoError { message: format!("Failed to spawn command: {}", e) }),
        Ok(Ok(output)) => {
            let raw_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let raw_stderr = String::from_utf8_lossy(&output.stderr).into_owned();

            let (stdout, stderr, truncated) = truncate_outputs(raw_stdout, raw_stderr);

            Ok(RunCommandResult {
                exit_code:   output.status.code(),
                stdout,
                stderr,
                truncated,
                duration_ms,
                command:     params.command,
                working_dir: work_dir.display().to_string(),
            })
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Truncate stdout + stderr so combined length ≤ TRUNCATE_CHARS.
/// Truncation is applied to the longer of the two first.
fn truncate_outputs(mut stdout: String, mut stderr: String) -> (String, String, bool) {
    let total = stdout.len() + stderr.len();
    if total <= TRUNCATE_CHARS {
        return (stdout, stderr, false);
    }

    // Give stdout up to half, rest to stderr
    let half = TRUNCATE_CHARS / 2;
    let mut truncated = false;

    if stdout.len() > half {
        stdout.truncate(half);
        stdout.push_str("\n...[truncated]");
        truncated = true;
    }
    let stderr_budget = TRUNCATE_CHARS - stdout.len();
    if stderr.len() > stderr_budget {
        stderr.truncate(stderr_budget);
        stderr.push_str("\n...[truncated]");
        truncated = true;
    }

    (stdout, stderr, truncated)
}
