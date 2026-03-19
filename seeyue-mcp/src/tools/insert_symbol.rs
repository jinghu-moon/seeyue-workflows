// src/tools/insert_symbol.rs
//
// sy_insert_after_symbol / sy_insert_before_symbol: insert content relative to a symbol.
// insert_after: inserts after end_line.
// insert_before: inserts before start_line.
// Both use atomic write (tmp → rename).

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::find_symbol::{run_find_symbol, FindSymbolParams};
use crate::tools::read::resolve_path;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InsertSymbolParams {
    pub name_path:     String,
    pub relative_path: String,
    pub content:       String,
}

// ─── Response ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InsertSymbolResult {
    #[serde(rename = "type")]
    pub kind:        String,
    pub insert_line: usize,
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub async fn run_insert_after_symbol(
    params: InsertSymbolParams,
    state:  &AppState,
) -> Result<InsertSymbolResult, ToolError> {
    insert_at_symbol(params, state, InsertPosition::After).await
}

pub async fn run_insert_before_symbol(
    params: InsertSymbolParams,
    state:  &AppState,
) -> Result<InsertSymbolResult, ToolError> {
    insert_at_symbol(params, state, InsertPosition::Before).await
}

// ─── Internal ────────────────────────────────────────────────────────────────

enum InsertPosition { After, Before }

async fn insert_at_symbol(
    params: InsertSymbolParams,
    state:  &AppState,
    pos:    InsertPosition,
) -> Result<InsertSymbolResult, ToolError> {
    let path = resolve_path(&state.workspace, &params.relative_path)?;
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.relative_path.clone(),
            hint: format!("File '{}' does not exist.", params.relative_path),
        });
    }

    // Find the symbol
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

    let file_data = safe_read(&path)?;
    let content   = file_data.content;
    let lines: Vec<&str> = content.lines().collect();

    let insert_after_line = match pos {
        InsertPosition::After  => target.end_line,   // insert after end_line
        InsertPosition::Before => target.start_line.saturating_sub(1), // insert before start_line
    };

    // Build new lines: [0..insert_after_line] + new_content_lines + [insert_after_line..]
    let mut result_lines: Vec<&str> = Vec::new();
    result_lines.extend_from_slice(&lines[..insert_after_line.min(lines.len())]);
    let new_lines: Vec<&str> = params.content.lines().collect();
    result_lines.extend_from_slice(&new_lines);
    if insert_after_line < lines.len() {
        result_lines.extend_from_slice(&lines[insert_after_line..]);
    }

    let new_content = if content.ends_with('\n') {
        format!("{}\n", result_lines.join("\n"))
    } else {
        result_lines.join("\n")
    };

    // Atomic write
    let tmp = { let mut p = path.clone().into_os_string(); p.push(".tmp"); std::path::PathBuf::from(p) };
    std::fs::write(&tmp, &new_content).map_err(|e| ToolError::IoError {
        message: format!("Failed to write tmp: {e}"),
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| ToolError::IoError {
        message: format!("Failed to rename tmp: {e}"),
    })?;

    Ok(InsertSymbolResult {
        kind:        "success".into(),
        insert_line: insert_after_line + 1,
    })
}
