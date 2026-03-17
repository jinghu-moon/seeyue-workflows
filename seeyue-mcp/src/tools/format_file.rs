// src/tools/format_file.rs
//
// Format a single file in-place using the appropriate formatter:
//   Rust   → rustfmt
//   Python → black / ruff format
//   JS/TS  → prettier
//   Go     → gofmt
// Falls back to "UNSUPPORTED" status when no formatter is available.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

use crate::error::ToolError;
use crate::treesitter::languages;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FormatFileParams {
    pub path: String,
    /// If true, only check whether the file is formatted — do not write
    pub check_only: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct FormatFileResult {
    #[serde(rename = "type")]
    pub kind:        String, // "success"
    pub path:        String,
    pub formatter:   String,
    pub status:      String, // "formatted" | "already_formatted" | "UNSUPPORTED" | "FORMATTER_NOT_FOUND"
    pub check_only:  bool,
    pub duration_ms: u64,
}

// ─── Formatter detection ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Formatter {
    Rustfmt,
    Black,
    Prettier,
    Gofmt,
}

impl Formatter {
    fn name(self) -> &'static str {
        match self {
            Formatter::Rustfmt  => "rustfmt",
            Formatter::Black    => "black",
            Formatter::Prettier => "prettier",
            Formatter::Gofmt    => "gofmt",
        }
    }

    fn detect(language: &str) -> Option<Self> {
        match language {
            "rust"       => Some(Formatter::Rustfmt),
            "python"     => Some(Formatter::Black),
            "javascript"
            | "typescript"
            | "tsx"
            | "jsx"      => Some(Formatter::Prettier),
            "go"         => Some(Formatter::Gofmt),
            _            => None,
        }
    }

    fn is_available(self) -> bool {
        which::which(self.name()).is_ok()
    }
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_format_file(
    params: FormatFileParams,
    workspace: &Path,
) -> Result<FormatFileResult, ToolError> {
    let abs = crate::tools::read::resolve_path(workspace, &params.path)?;
    if !abs.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let check_only = params.check_only.unwrap_or(false);
    let language = languages::detect_language(&abs);

    let Some(formatter) = Formatter::detect(&language) else {
        return Ok(FormatFileResult {
            kind:        "success".into(),
            path:        params.path,
            formatter:   "none".into(),
            status:      "UNSUPPORTED".into(),
            check_only,
            duration_ms: 0,
        });
    };

    if !formatter.is_available() {
        return Ok(FormatFileResult {
            kind:        "success".into(),
            path:        params.path,
            formatter:   formatter.name().into(),
            status:      "FORMATTER_NOT_FOUND".into(),
            check_only,
            duration_ms: 0,
        });
    }

    let start = Instant::now();
    let status = run_formatter(formatter, &abs, check_only)?;
    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(FormatFileResult {
        kind: "success".into(),
        path: params.path,
        formatter: formatter.name().into(),
        status,
        check_only,
        duration_ms,
    })
}

fn run_formatter(fmt: Formatter, path: &Path, check_only: bool) -> Result<String, ToolError> {
    let path_str = path.to_string_lossy();

    let (program, args): (&str, Vec<String>) = match fmt {
        Formatter::Rustfmt => {
            let mut a = vec![path_str.to_string()];
            if check_only { a.push("--check".into()); }
            ("rustfmt", a)
        }
        Formatter::Black => {
            let mut a = vec![path_str.to_string()];
            if check_only { a.push("--check".into()); }
            ("black", a)
        }
        Formatter::Prettier => {
            let a = if check_only {
                vec!["--check".into(), path_str.to_string()]
            } else {
                vec!["--write".into(), path_str.to_string()]
            };
            ("prettier", a)
        }
        Formatter::Gofmt => {
            let a = if check_only {
                vec!["-l".into(), path_str.to_string()]
            } else {
                vec!["-w".into(), path_str.to_string()]
            };
            ("gofmt", a)
        }
    };

    let output = std::process::Command::new(program)
        .args(&args)
        .output()
        .map_err(|e| ToolError::IoError { message: format!("Failed to run {program}: {e}") })?;

    if check_only {
        // exit 0 = already formatted, non-zero = needs formatting
        Ok(if output.status.success() { "already_formatted" } else { "needs_formatting" }.into())
    } else {
        Ok(if output.status.success() { "formatted" } else {
            // formatter exited non-zero — surface stderr
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::IoError {
                message: format!("{program} failed: {stderr}"),
            });
        }.into())
    }
}
