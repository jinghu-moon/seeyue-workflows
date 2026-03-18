// src/tools/run_script.rs
//
// run_script: Execute a script file (.ps1 / .sh / .py / .js / .ts)
// with appropriate interpreter. Builds on run_command's execution model.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RunScriptParams {
    /// Script file path relative to workspace root.
    pub script:      String,
    /// Arguments to pass to the script.
    pub args:        Option<Vec<String>>,
    /// Working directory relative to workspace root (default: workspace root).
    pub working_dir: Option<String>,
    /// Timeout in seconds (default: 30, max: 300).
    pub timeout_secs: Option<u64>,
    /// Environment variables to set.
    pub env:         Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct RunScriptResult {
    #[serde(rename = "type")]
    pub kind:        String, // "success" | "error" | "timeout"
    pub script:      String,
    pub interpreter: String,
    pub exit_code:   i32,
    pub stdout:      String,
    pub stderr:      String,
    pub duration_ms: u64,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub async fn run_script(
    params: RunScriptParams,
    state: &AppState,
) -> Result<RunScriptResult, ToolError> {
    if params.script.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "script".into(),
            hint:    "Provide a script file path (e.g. scripts/build.ps1).".into(),
        });
    }

    let workspace    = state.workspace.as_ref();
    let script_path  = workspace.join(&params.script);

    if !script_path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.script.clone(),
            hint:      "Script file does not exist.".into(),
        });
    }

    let ext = script_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let (interpreter, interp_args) = resolve_interpreter(&ext)?;

    let timeout_secs = params.timeout_secs.unwrap_or(30).min(300);
    let cwd = params.working_dir
        .as_ref()
        .map(|d| workspace.join(d))
        .unwrap_or_else(|| workspace.to_path_buf());

    let script_abs = script_path.to_string_lossy().to_string();
    let user_args  = params.args.clone().unwrap_or_default();
    let env_vars   = params.env.clone().unwrap_or_default();

    let start = std::time::Instant::now();

    let mut cmd = tokio::process::Command::new(&interpreter);
    cmd.args(&interp_args)
       .arg(&script_abs)
       .args(&user_args)
       .current_dir(&cwd)
       .envs(&env_vars)
       .stdout(std::process::Stdio::piped())
       .stderr(std::process::Stdio::piped());

    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        cmd.output(),
    ).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout    = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr    = String::from_utf8_lossy(&output.stderr).to_string();
            let kind      = if exit_code == 0 { "success" } else { "error" };
            Ok(RunScriptResult {
                kind:        kind.into(),
                script:      params.script,
                interpreter,
                exit_code,
                stdout:      truncate_output(&stdout, 8192),
                stderr:      truncate_output(&stderr, 4096),
                duration_ms,
            })
        }
        Ok(Err(e)) => Err(ToolError::IoError {
            message: format!("spawn script: {e}"),
        }),
        Err(_) => Ok(RunScriptResult {
            kind:        "timeout".into(),
            script:      params.script,
            interpreter,
            exit_code:   -1,
            stdout:      String::new(),
            stderr:      format!("Script timed out after {}s", timeout_secs),
            duration_ms,
        }),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn resolve_interpreter(ext: &str) -> Result<(String, Vec<String>), ToolError> {
    match ext {
        "ps1" => Ok(("powershell.exe".into(), vec!["-NoProfile".into(), "-File".into()])),
        "sh" | "bash" => {
            // Prefer bash, fall back to sh
            if which_exists("bash") {
                Ok(("bash".into(), vec![]))
            } else {
                Ok(("sh".into(), vec![]))
            }
        }
        "py" | "python" => {
            if which_exists("python3") {
                Ok(("python3".into(), vec![]))
            } else {
                Ok(("python".into(), vec![]))
            }
        }
        "js" | "mjs" => {
            if which_exists("node") {
                Ok(("node".into(), vec![]))
            } else {
                Err(ToolError::IoError {
                    message: "node not found. Install Node.js to run .js scripts.".into(),
                })
            }
        }
        "ts" => {
            if which_exists("tsx") {
                Ok(("tsx".into(), vec![]))
            } else if which_exists("ts-node") {
                Ok(("ts-node".into(), vec![]))
            } else {
                Err(ToolError::UnsupportedLanguage {
                    language: "typescript".into(),
                    hint:     "Install tsx (`npm i -g tsx`) to run .ts scripts.".into(),
                })
            }
        }
        _ => Err(ToolError::UnsupportedLanguage {
            language: ext.to_string(),
            hint:     "Supported extensions: .ps1 .sh .bash .py .js .mjs .ts".into(),
        }),
    }
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn truncate_output(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes { return s.to_string(); }
    let start = s.len().saturating_sub(max_bytes);
    format!("[...truncated]\n{}", &s[start..])
}
