// src/tools/lint_file.rs
//
// Semantic linter integration: clippy / eslint / ruff.
// Returns structured diagnostics suitable for Agent consumption.

use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::error::ToolError;

// ─── Constants ───────────────────────────────────────────────────────────────

const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const MAX_RESULTS: usize = 50;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct LintFileParams {
    pub path:   String,
    pub linter: Option<String>,
    pub fix:    Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LintFileResult {
    pub linter:       String,
    pub path:         String,
    pub diagnostics:  Vec<Diagnostic>,
    pub total_issues: usize,
    pub truncated:    bool,
    pub fixed:        Option<u32>,
    pub duration_ms:  u64,
}

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub severity: String,   // "error" | "warning" | "info"
    pub rule:     String,
    pub message:  String,
    pub line:     Option<u32>,
    pub column:   Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
enum Linter { Clippy, Eslint, Ruff }

impl Linter {
    fn label(&self) -> &'static str {
        match self {
            Linter::Clippy => "cargo clippy",
            Linter::Eslint => "eslint",
            Linter::Ruff   => "ruff",
        }
    }
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub async fn run_lint_file(
    params: LintFileParams,
    workspace: &Path,
) -> Result<LintFileResult, ToolError> {
    let linter = detect_linter(&params.linter, &params.path, workspace)?;
    let fix = params.fix.unwrap_or(false);

    let abs_path = crate::platform::path::resolve(workspace, &params.path)
        .map_err(|e| ToolError::PathEscape {
            file_path: params.path.clone(),
            hint: format!("{:?}", e),
        })?;

    let start = Instant::now();
    let raw = run_linter_command(&linter, &abs_path.to_string_lossy(), workspace, fix).await?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let (diagnostics, fixed) = parse_output(&linter, &raw, fix);
    let total_issues = diagnostics.len();
    let truncated = total_issues > MAX_RESULTS;
    let diagnostics = diagnostics.into_iter().take(MAX_RESULTS).collect();

    Ok(LintFileResult {
        linter: linter.label().to_string(),
        path: params.path,
        diagnostics,
        total_issues,
        truncated,
        fixed,
        duration_ms,
    })
}

// ─── Linter Detection ────────────────────────────────────────────────────────

fn detect_linter(
    hint: &Option<String>,
    path: &str,
    workspace: &Path,
) -> Result<Linter, ToolError> {
    if let Some(h) = hint {
        return match h.to_lowercase().as_str() {
            "clippy" | "rust" | "cargo" => Ok(Linter::Clippy),
            "eslint" | "js" | "ts"      => Ok(Linter::Eslint),
            "ruff" | "python"           => Ok(Linter::Ruff),
            other => Err(ToolError::UnsupportedLanguage {
                language: other.to_string(),
                hint: "Supported linters: clippy, eslint, ruff".to_string(),
            }),
        };
    }

    // Auto-detect from extension
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs"  => Ok(Linter::Clippy),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Ok(Linter::Eslint),
        "py"  => Ok(Linter::Ruff),
        _ => {
            // Fall back to workspace root detection
            if workspace.join("Cargo.toml").exists() {
                return Ok(Linter::Clippy);
            }
            if workspace.join("package.json").exists() {
                return Ok(Linter::Eslint);
            }
            Err(ToolError::UnsupportedLanguage {
                language: ext.to_string(),
                hint: "Cannot detect linter. Use linter param: clippy|eslint|ruff".to_string(),
            })
        }
    }
}

// ─── Command Execution ───────────────────────────────────────────────────────

async fn run_linter_command(
    linter: &Linter,
    abs_path: &str,
    workspace: &Path,
    fix: bool,
) -> Result<String, ToolError> {
    let args: Vec<String> = match linter {
        Linter::Clippy => {
            let mut a = vec![
                "cargo".to_string(),
                "clippy".to_string(),
                "--message-format".to_string(),
                "json".to_string(),
                "--quiet".to_string(),
            ];
            if fix {
                a.push("--fix".to_string());
                a.push("--allow-dirty".to_string());
            }
            a
        }
        Linter::Eslint => {
            let mut a = vec![
                "npx".to_string(),
                "eslint".to_string(),
                "--format".to_string(),
                "json".to_string(),
                abs_path.to_string(),
            ];
            if fix { a.push("--fix".to_string()); }
            a
        }
        Linter::Ruff => {
            let mut a = vec![
                "ruff".to_string(),
                "check".to_string(),
                "--output-format".to_string(),
                "json".to_string(),
                abs_path.to_string(),
            ];
            if fix { a.push("--fix".to_string()); }
            a
        }
    };

    let program = args[0].clone();
    let rest = &args[1..];

    let output = timeout(
        Duration::from_millis(DEFAULT_TIMEOUT_MS),
        Command::new(&program)
            .args(rest)
            .current_dir(workspace)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| ToolError::IoError {
        message: format!("{} timed out", linter.label()),
    })?
    .map_err(|e| ToolError::IoError {
        message: format!("Failed to spawn {}: {}", linter.label(), e),
    })?;

    // Linters exit non-zero when issues found — that's expected, not an error
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok(format!("{}{}", stdout, stderr))
}

// ─── Output Parsing ──────────────────────────────────────────────────────────

fn parse_output(
    linter: &Linter,
    raw: &str,
    fix: bool,
) -> (Vec<Diagnostic>, Option<u32>) {
    match linter {
        Linter::Clippy => parse_clippy(raw),
        Linter::Eslint => parse_eslint(raw, fix),
        Linter::Ruff   => parse_ruff(raw),
    }
}

/// Parse `cargo clippy --message-format json` (newline-delimited JSON objects).
fn parse_clippy(raw: &str) -> (Vec<Diagnostic>, Option<u32>) {
    let mut diags = Vec::new();
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if v.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") { continue; }
        let msg = &v["message"];
        let level = msg.get("level").and_then(|l| l.as_str()).unwrap_or("warning");
        let text  = msg.get("message").and_then(|m| m.as_str()).unwrap_or("");
        let code  = msg.get("code").and_then(|c| c.get("code")).and_then(|c| c.as_str()).unwrap_or("");
        let (line_no, col_no) = first_span_location(msg);
        diags.push(Diagnostic {
            severity: level.to_string(),
            rule:     code.to_string(),
            message:  text.to_string(),
            line:     line_no,
            column:   col_no,
        });
    }
    (diags, None)
}

/// Parse `eslint --format json` (JSON array of file objects).
fn parse_eslint(raw: &str, fix: bool) -> (Vec<Diagnostic>, Option<u32>) {
    let Ok(arr) = serde_json::from_str::<serde_json::Value>(raw) else {
        return (Vec::new(), None);
    };
    let files = arr.as_array().cloned().unwrap_or_default();
    let mut diags = Vec::new();
    let mut fixed_count = 0u32;
    for file in &files {
        if fix {
            fixed_count += file.get("fixableErrorCount")
                .and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            fixed_count += file.get("fixableWarningCount")
                .and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        }
        for msg in file.get("messages").and_then(|m| m.as_array()).unwrap_or(&vec![]) {
            let severity = match msg.get("severity").and_then(|s| s.as_u64()) {
                Some(2) => "error",
                Some(1) => "warning",
                _       => "info",
            };
            diags.push(Diagnostic {
                severity: severity.to_string(),
                rule:     msg.get("ruleId").and_then(|r| r.as_str()).unwrap_or("").to_string(),
                message:  msg.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string(),
                line:     msg.get("line").and_then(|l| l.as_u64()).map(|l| l as u32),
                column:   msg.get("column").and_then(|c| c.as_u64()).map(|c| c as u32),
            });
        }
    }
    let fixed = if fix { Some(fixed_count) } else { None };
    (diags, fixed)
}

/// Parse `ruff check --output-format json` (JSON array).
fn parse_ruff(raw: &str) -> (Vec<Diagnostic>, Option<u32>) {
    let Ok(arr) = serde_json::from_str::<serde_json::Value>(raw) else {
        return (Vec::new(), None);
    };
    let items = arr.as_array().cloned().unwrap_or_default();
    let diags = items.iter().map(|item| Diagnostic {
        severity: "warning".to_string(),
        rule:     item.get("code").and_then(|c| c.as_str()).unwrap_or("").to_string(),
        message:  item.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string(),
        line:     item.pointer("/location/row").and_then(|v| v.as_u64()).map(|v| v as u32),
        column:   item.pointer("/location/column").and_then(|v| v.as_u64()).map(|v| v as u32),
    }).collect();
    (diags, None)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn first_span_location(msg: &serde_json::Value) -> (Option<u32>, Option<u32>) {
    let spans = msg.get("spans").and_then(|s| s.as_array());
    if let Some(spans) = spans {
        if let Some(span) = spans.first() {
            let line = span.get("line_start").and_then(|v| v.as_u64()).map(|v| v as u32);
            let col  = span.get("column_start").and_then(|v| v.as_u64()).map(|v| v as u32);
            return (line, col);
        }
    }
    (None, None)
}
