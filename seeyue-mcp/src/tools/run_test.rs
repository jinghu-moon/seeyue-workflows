// src/tools/run_test.rs
//
// TDD-loop test runner: auto-detects language runner, executes tests,
// and applies two-phase output filtering to reduce noise.
//
// Supported runners:
//   Rust    — cargo test [filter]
//   Node.js — npx jest / npx vitest run [filter]
//   Python  — pytest [filter]

use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::error::ToolError;

// ─── Constants ───────────────────────────────────────────────────────────────

const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const TRUNCATE_CHARS: usize = 8_000;

// Hard-exclude patterns (phase-1 filter): discard lines matching these.
// Based on claude-code-security-review two-phase filtering approach.
const HARD_EXCLUDE: &[&str] = &[
    "DEPRECATION WARNING",
    "ExperimentalWarning",
    "DeprecationWarning",
    "punycode",
    "--experimental-",
    "npm warn",
    "npm notice",
];

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct RunTestParams {
    pub filter:     Option<String>,
    pub language:   Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RunTestResult {
    pub passed:         bool,
    pub exit_code:      Option<i32>,
    pub runner:         String,
    pub stdout:         String,
    pub stderr:         String,
    pub filtered_lines: u32,
    pub truncated:      bool,
    pub duration_ms:    u64,
    #[serde(default)]
    pub elapsed_ms:     u64,
}

#[derive(Debug, Clone, PartialEq)]
enum Runner {
    Cargo,
    Jest,
    Vitest,
    Pytest,
}

impl Runner {
    fn label(&self) -> &'static str {
        match self {
            Runner::Cargo  => "cargo test",
            Runner::Jest   => "npx jest",
            Runner::Vitest => "npx vitest run",
            Runner::Pytest => "pytest",
        }
    }
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub async fn run_run_test(
    params: RunTestParams,
    workspace: &Path,
) -> Result<RunTestResult, ToolError> {
    let runner = detect_runner(&params.language, workspace)?;
    let timeout_ms = params
        .timeout_ms
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .min(MAX_TIMEOUT_MS);

    let mut args = build_args(&runner, &params.filter);
    let program = args.remove(0);

    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .current_dir(workspace)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let start = Instant::now();
    let output_result = timeout(Duration::from_millis(timeout_ms), cmd.output()).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match output_result {
        Err(_) => Ok(RunTestResult {
            passed:         false,
            exit_code:      None,
            runner:         runner.label().to_string(),
            stdout:         String::new(),
            stderr:         format!("Test run timed out after {}ms", timeout_ms),
            filtered_lines: 0,
            truncated:      false,
            duration_ms,
            elapsed_ms:     duration_ms,
        }),
        Ok(Err(e)) => Err(ToolError::IoError {
            message: format!("Failed to spawn test runner '{}': {}", program, e),
        }),
        Ok(Ok(output)) => {
            let raw_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let raw_stderr = String::from_utf8_lossy(&output.stderr).into_owned();

            let (stdout, stderr, filtered_lines, truncated) =
                filter_and_truncate(raw_stdout, raw_stderr);

            let passed = output.status.success();

            Ok(RunTestResult {
                passed,
                exit_code: output.status.code(),
                runner:    runner.label().to_string(),
                stdout,
                stderr,
                filtered_lines,
                truncated,
                duration_ms,
                elapsed_ms: duration_ms,
            })
        }
    }
}

// ─── Runner Detection ────────────────────────────────────────────────────────

fn detect_runner(
    hint: &Option<String>,
    workspace: &Path,
) -> Result<Runner, ToolError> {
    // Explicit hint takes priority
    if let Some(lang) = hint {
        return match lang.to_lowercase().as_str() {
            "rust" | "cargo"          => Ok(Runner::Cargo),
            "jest"                    => Ok(Runner::Jest),
            "vitest"                  => Ok(Runner::Vitest),
            "python" | "pytest"       => Ok(Runner::Pytest),
            "typescript" | "js" | "node" => detect_node_runner(workspace),
            other => Err(ToolError::UnsupportedLanguage {
                language: other.to_string(),
                hint: "Supported: rust, jest, vitest, typescript, python".to_string(),
            }),
        };
    }

    // Auto-detect from workspace root files
    if workspace.join("Cargo.toml").exists() {
        return Ok(Runner::Cargo);
    }
    if workspace.join("pyproject.toml").exists() || workspace.join("setup.py").exists() {
        return Ok(Runner::Pytest);
    }
    if workspace.join("package.json").exists() {
        return detect_node_runner(workspace);
    }

    Err(ToolError::UnsupportedLanguage {
        language: "unknown".to_string(),
        hint: "Cannot detect test runner. Use language param: rust|jest|vitest|python".to_string(),
    })
}

/// Detect jest vs vitest from package.json scripts/devDependencies.
fn detect_node_runner(workspace: &Path) -> Result<Runner, ToolError> {
    let pkg_path = workspace.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        if content.contains("vitest") {
            return Ok(Runner::Vitest);
        }
        if content.contains("jest") {
            return Ok(Runner::Jest);
        }
    }
    // Default to jest if package.json exists but no clear signal
    Ok(Runner::Jest)
}

// ─── Arg Building ────────────────────────────────────────────────────────────

fn build_args(runner: &Runner, filter: &Option<String>) -> Vec<String> {
    match runner {
        Runner::Cargo => {
            let mut args = vec!["cargo".to_string(), "test".to_string()];
            if let Some(f) = filter {
                args.push(f.clone());
            }
            args
        }
        Runner::Jest => {
            let mut args = vec![
                "npx".to_string(),
                "jest".to_string(),
                "--no-coverage".to_string(),
            ];
            if let Some(f) = filter {
                args.push("--testNamePattern".to_string());
                args.push(f.clone());
            }
            args
        }
        Runner::Vitest => {
            let mut args = vec![
                "npx".to_string(),
                "vitest".to_string(),
                "run".to_string(),
            ];
            if let Some(f) = filter {
                args.push("--reporter=verbose".to_string());
                args.push("-t".to_string());
                args.push(f.clone());
            }
            args
        }
        Runner::Pytest => {
            let mut args = vec!["pytest".to_string(), "-v".to_string()];
            if let Some(f) = filter {
                args.push("-k".to_string());
                args.push(f.clone());
            }
            args
        }
    }
}

// ─── Two-Phase Output Filtering ──────────────────────────────────────────────

/// Phase 1: hard-exclude noise lines.
/// Phase 2: truncate combined output to TRUNCATE_CHARS.
fn filter_and_truncate(
    stdout: String,
    stderr: String,
) -> (String, String, u32, bool) {
    let (filtered_stdout, n1) = filter_lines(stdout);
    let (filtered_stderr, n2) = filter_lines(stderr);
    let filtered_lines = n1 + n2;

    let total = filtered_stdout.len() + filtered_stderr.len();
    if total <= TRUNCATE_CHARS {
        return (filtered_stdout, filtered_stderr, filtered_lines, false);
    }

    let half = TRUNCATE_CHARS / 2;
    let mut out = filtered_stdout;
    let mut err = filtered_stderr;
    let mut truncated = false;

    if out.len() > half {
        out.truncate(half);
        out.push_str("\n...[truncated]");
        truncated = true;
    }
    let err_budget = TRUNCATE_CHARS - out.len();
    if err.len() > err_budget {
        err.truncate(err_budget);
        err.push_str("\n...[truncated]");
        truncated = true;
    }

    (out, err, filtered_lines, truncated)
}

fn filter_lines(input: String) -> (String, u32) {
    let mut kept = Vec::new();
    let mut dropped = 0u32;
    for line in input.lines() {
        if HARD_EXCLUDE.iter().any(|pat| line.contains(pat)) {
            dropped += 1;
        } else {
            kept.push(line);
        }
    }
    (kept.join("\n"), dropped)
}
