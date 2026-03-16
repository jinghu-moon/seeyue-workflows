// src/tools/type_check.rs
//
// TypeScript/Python type checking.
// TypeScript: npx tsc --noEmit (or tsc if installed globally)
// Python:     mypy [path] --output=json
// Returns TOOL_NOT_FOUND with install hint if checker not available.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct TypeCheckParams {
    pub path:     String,           // file or directory
    pub language: Option<String>,   // "typescript" | "python" (auto-detected)
}

#[derive(Debug, Serialize)]
pub struct TypeCheckResult {
    pub status:       String,   // "ok" | "errors" | "TOOL_NOT_FOUND"
    pub language:     String,
    pub tool:         String,   // "tsc" | "mypy"
    pub error_count:  usize,
    pub errors:       Vec<TypeCheckIssue>,
    pub raw_output:   Option<String>,
    pub install_hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TypeCheckIssue {
    pub file:    String,
    pub line:    Option<u32>,
    pub column:  Option<u32>,
    pub message: String,
    pub code:    Option<String>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_type_check(
    params:    TypeCheckParams,
    workspace: &Path,
) -> Result<TypeCheckResult, ToolError> {
    let abs = crate::platform::path::resolve(workspace, &params.path)
        .map_err(|e| ToolError::PathEscape {
            file_path: params.path.clone(),
            hint:      format!("{:?}", e),
        })?;

    if !abs.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint:      "Path does not exist.".to_string(),
        });
    }

    let language = detect_language(&abs, params.language.as_deref());

    match language.as_str() {
        "typescript" => run_tsc(&abs, workspace),
        "python"     => run_mypy(&abs, workspace),
        _ => Err(ToolError::UnsupportedLanguage {
            language,
            hint: "Supported languages: typescript, python.".to_string(),
        }),
    }
}

// ─── Language Detection ──────────────────────────────────────────────────────

fn detect_language(path: &Path, hint: Option<&str>) -> String {
    if let Some(h) = hint {
        return h.to_lowercase();
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "ts" | "tsx" => "typescript".to_string(),
        "py"         => "python".to_string(),
        _            => {
            // Directory: check for tsconfig.json or pyproject.toml
            if path.join("tsconfig.json").exists() {
                "typescript".to_string()
            } else if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
                "python".to_string()
            } else {
                "unknown".to_string()
            }
        }
    }
}

// ─── TypeScript (tsc) ────────────────────────────────────────────────────────

fn run_tsc(path: &Path, workspace: &Path) -> Result<TypeCheckResult, ToolError> {
    // Try tsc directly, then npx tsc
    let tsc_bin = which::which("tsc").ok();
    let (program, args): (&str, Vec<&str>) = if tsc_bin.is_some() {
        ("tsc", vec!["--noEmit", "--pretty", "false"])
    } else {
        // Check npx is available
        if which::which("npx").is_err() {
            return Ok(tool_not_found(
                "typescript", "tsc",
                "Install TypeScript: npm install -g typescript",
            ));
        }
        ("npx", vec!["tsc", "--noEmit", "--pretty", "false"])
    };

    // Run from workspace or path directory
    let cwd = if path.is_dir() { path } else { workspace };

    let output = Command::new(program)
        .args(&args)
        .current_dir(cwd)
        .output();

    let output = match output {
        Ok(o)  => o,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(tool_not_found(
                    "typescript", "tsc",
                    "Install TypeScript: npm install -g typescript",
                ));
            }
            return Err(ToolError::IoError { message: e.to_string() });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}{}", stdout, stderr);

    let errors = parse_tsc_output(&combined);
    let error_count = errors.len();
    let status = if error_count == 0 { "ok" } else { "errors" };

    Ok(TypeCheckResult {
        status:       status.to_string(),
        language:     "typescript".to_string(),
        tool:         "tsc".to_string(),
        error_count,
        errors,
        raw_output:   if combined.trim().is_empty() { None } else { Some(truncate(&combined, 5000)) },
        install_hint: None,
    })
}

/// Parse tsc output lines like: src/foo.ts(10,5): error TS2345: ...
fn parse_tsc_output(output: &str) -> Vec<TypeCheckIssue> {
    let re = regex::Regex::new(
        r"^(.+?)\((\d+),(\d+)\):\s*(error|warning)\s+(TS\d+):\s*(.+)$"
    ).unwrap();
    output.lines()
        .filter_map(|line| {
            let cap = re.captures(line)?;
            Some(TypeCheckIssue {
                file:    cap[1].to_string(),
                line:    cap[2].parse().ok(),
                column:  cap[3].parse().ok(),
                message: cap[6].to_string(),
                code:    Some(cap[5].to_string()),
            })
        })
        .take(50)
        .collect()
}

// ─── Python (mypy) ───────────────────────────────────────────────────────────

fn run_mypy(path: &Path, _workspace: &Path) -> Result<TypeCheckResult, ToolError> {
    if which::which("mypy").is_err() {
        return Ok(tool_not_found(
            "python", "mypy",
            "Install mypy: pip install mypy",
        ));
    }

    let output = Command::new("mypy")
        .args([path.to_string_lossy().as_ref(), "--no-error-summary"])
        .output();

    let output = match output {
        Ok(o)  => o,
        Err(e) => return Err(ToolError::IoError { message: e.to_string() }),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let errors = parse_mypy_output(&stdout);
    let error_count = errors.len();
    let status = if error_count == 0 { "ok" } else { "errors" };

    Ok(TypeCheckResult {
        status:       status.to_string(),
        language:     "python".to_string(),
        tool:         "mypy".to_string(),
        error_count,
        errors,
        raw_output:   if stdout.trim().is_empty() { None } else { Some(truncate(&stdout, 5000)) },
        install_hint: None,
    })
}

/// Parse mypy output lines like: src/foo.py:10: error: ...
fn parse_mypy_output(output: &str) -> Vec<TypeCheckIssue> {
    let re = regex::Regex::new(
        r"^(.+?):(\d+):\s*(error|warning|note):\s*(.+)$"
    ).unwrap();
    output.lines()
        .filter_map(|line| {
            let cap = re.captures(line)?;
            let severity = &cap[3];
            if severity == "note" { return None; }  // skip notes
            Some(TypeCheckIssue {
                file:    cap[1].to_string(),
                line:    cap[2].parse().ok(),
                column:  None,
                message: cap[4].to_string(),
                code:    None,
            })
        })
        .take(50)
        .collect()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn tool_not_found(language: &str, tool: &str, hint: &str) -> TypeCheckResult {
    TypeCheckResult {
        status:       "TOOL_NOT_FOUND".to_string(),
        language:     language.to_string(),
        tool:         tool.to_string(),
        error_count:  0,
        errors:       Vec::new(),
        raw_output:   None,
        install_hint: Some(hint.to_string()),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}... [truncated]", &s[..max])
    }
}
