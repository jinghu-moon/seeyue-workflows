// src/tools/replace_symbol_body.rs
//
// sy_replace_symbol_body: replace the body of a symbol in-place.
// Uses atomic write (tmp → rename) to avoid partial writes.

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::find_symbol::{run_find_symbol, FindSymbolParams};
use crate::tools::read::resolve_path;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReplaceSymbolBodyParams {
    /// name_path of the symbol to replace (e.g. "Greeter/greet")
    pub name_path:     String,
    /// File containing the symbol (relative path)
    pub relative_path: String,
    /// Complete new body, including the function/method signature line
    pub new_body:      String,
}

// ─── Response ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ReplaceSymbolBodyResult {
    #[serde(rename = "type")]
    pub kind:         String, // "success"
    pub lines_changed: usize,
    pub start_line:   usize,
    pub end_line:     usize,
}

// ─── Main logic ───────────────────────────────────────────────────────────────

pub async fn run_replace_symbol_body(
    params: ReplaceSymbolBodyParams,
    state:  &AppState,
) -> Result<ReplaceSymbolBodyResult, ToolError> {
    let path = resolve_path(&state.workspace, &params.relative_path)?;
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.relative_path.clone(),
            hint: format!("File '{}' does not exist.", params.relative_path),
        });
    }

    // 1. Find the symbol to get its line range
    let sym_result = run_find_symbol(
        FindSymbolParams {
            name_path_pattern:  params.name_path.clone(),
            relative_path:      Some(params.relative_path.clone()),
            substring_matching: Some(false),
            include_body:       Some(false),
            depth:              Some(2),
        },
        state,
    ).await?;

    let target = sym_result.matches.into_iter().next().ok_or_else(|| ToolError::FileNotFound {
        file_path: params.name_path.clone(),
        hint: format!("Symbol '{}' not found in '{}'", params.name_path, params.relative_path),
    })?;

    let start_line = target.start_line; // 1-indexed
    let end_line   = target.end_line;   // 1-indexed

    // 2. Read the file content
    let file_data = safe_read(&path)?;
    let content   = file_data.content;
    let lines: Vec<&str> = content.lines().collect();

    if start_line == 0 || end_line == 0 || start_line > lines.len() {
        return Err(ToolError::IoError {
            message: format!("Invalid symbol line range: {}-{}", start_line, end_line),
        });
    }

    // 3. Build new content: lines before + new_body + lines after
    let before: Vec<&str> = lines[..start_line - 1].to_vec();
    let after:  Vec<&str> = if end_line < lines.len() {
        lines[end_line..].to_vec()
    } else {
        vec![]
    };

    let old_line_count = end_line - start_line + 1;
    let new_lines: Vec<&str> = params.new_body.lines().collect();
    let new_line_count = new_lines.len();

    let mut result_lines: Vec<&str> = Vec::new();
    result_lines.extend_from_slice(&before);
    result_lines.extend_from_slice(&new_lines);
    result_lines.extend_from_slice(&after);

    // Preserve trailing newline
    let new_content = if content.ends_with('\n') {
        format!("{}\n", result_lines.join("\n"))
    } else {
        result_lines.join("\n")
    };

    // 4. Atomic write
    let tmp = path.with_extension("rs.tmp");
    std::fs::write(&tmp, &new_content).map_err(|e| ToolError::IoError {
        message: format!("Failed to write tmp file: {e}"),
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| ToolError::IoError {
        message: format!("Failed to rename tmp to target: {e}"),
    })?;

    let lines_changed = (old_line_count as isize - new_line_count as isize).unsigned_abs();
    Ok(ReplaceSymbolBodyResult {
        kind:         "success".into(),
        lines_changed: lines_changed.max(1),
        start_line,
        end_line,
    })
}
