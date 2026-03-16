use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::treesitter::{languages, symbols};
use crate::tools::read::resolve_path;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReadRangeParams {
    pub path:          String,
    pub start:         Option<usize>,
    pub end:           Option<usize>,
    pub symbol:        Option<String>,
    pub context_lines: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ReadRangeResult {
    #[serde(rename = "type")]
    pub kind:         String, // "success"
    pub path:         String,
    pub start:        usize,
    pub end:          usize,
    pub symbol_start: Option<usize>,
    pub symbol_end:   Option<usize>,
    pub total_lines:  usize,
    pub content:      String,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_read_range(
    params: ReadRangeParams,
    workspace: &Path,
) -> Result<ReadRangeResult, ToolError> {
    let path = resolve_path(workspace, &params.path)?;

    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let content = file_data.content;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let context = params.context_lines.unwrap_or(0);

    let (raw_start, raw_end) = if let Some(sym) = params.symbol.as_deref() {
        resolve_symbol_range(&path, &content, sym)?
    } else {
        let start = params.start.ok_or_else(|| ToolError::MissingParameter {
            missing: "start|symbol".into(),
            hint: "Provide start/end or symbol to read a range.".into(),
        })?;
        let end = params.end.unwrap_or(start);
        (start, end)
    };

    if raw_start == 0 || raw_end == 0 || (raw_start > raw_end && total_lines > 0) {
        return Err(ToolError::InvalidLineRange {
            start_line: raw_start,
            end_line: raw_end,
            total_lines,
            hint: "start must be <= end and both must be >= 1.".into(),
        });
    }

    let effective_start = raw_start.saturating_sub(context).max(1);
    let effective_end = (raw_end + context).min(total_lines.max(1));

    let width = total_lines.to_string().len().max(3);
    let content = if total_lines == 0 {
        String::new()
    } else {
        lines[effective_start - 1..effective_end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>width$}\t{}", effective_start + i, line, width = width))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(ReadRangeResult {
        kind: "success".into(),
        path: params.path,
        start: effective_start,
        end: effective_end,
        symbol_start: if params.symbol.is_some() { Some(raw_start) } else { None },
        symbol_end: if params.symbol.is_some() { Some(raw_end) } else { None },
        total_lines,
        content,
    })
}

// ─── 符号解析 ────────────────────────────────────────────────────────────────

fn resolve_symbol_range(path: &Path, content: &str, symbol: &str) -> Result<(usize, usize), ToolError> {
    let language = languages::detect_language(path);
    let symbols = symbols::extract_symbols(&language, content, 2);

    for sym in symbols {
        if sym.name == symbol {
            return Ok((sym.line, sym.end_line));
        }
    }

    Err(ToolError::MissingParameter {
        missing: "symbol".into(),
        hint: format!("Symbol '{symbol}' not found. Use file_outline to list available symbols."),
    })
}
