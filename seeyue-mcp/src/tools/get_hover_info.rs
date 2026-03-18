// src/tools/get_hover_info.rs
//
// get_hover_info: LSP textDocument/hover — returns symbol type signature,
// documentation, and inferred type at a given position.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::read::resolve_path;
use crate::treesitter::languages;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GetHoverInfoParams {
    /// File path relative to workspace root.
    pub path:   String,
    /// 1-based line number.
    pub line:   usize,
    /// 1-based column number.
    pub column: usize,
}

#[derive(Debug, Serialize)]
pub struct HoverInfoResult {
    #[serde(rename = "type")]
    pub kind:       String, // "success" | "not_available" | "no_info"
    pub path:       String,
    pub line:       usize,
    pub column:     usize,
    /// Markdown-formatted hover content from the LSP server.
    pub contents:   Option<String>,
    /// Inferred type string (extracted from hover content when possible).
    pub type_hint:  Option<String>,
    /// Range the hover applies to (start_line, start_col, end_line, end_col).
    pub range:      Option<HoverRange>,
    pub source:     String, // "lsp" | "none"
}

#[derive(Debug, Serialize)]
pub struct HoverRange {
    pub start_line: usize,
    pub start_col:  usize,
    pub end_line:   usize,
    pub end_col:    usize,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub async fn run_get_hover_info(
    params: GetHoverInfoParams,
    state: &AppState,
) -> Result<HoverInfoResult, ToolError> {
    let path = resolve_path(&state.workspace, &params.path)?;
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let content   = file_data.content;
    let language  = languages::detect_language(&path);

    let hover_val = try_lsp_hover(&path, &content, &language, params.line, params.column, state).await;

    match hover_val {
        Some(val) => {
            let contents  = extract_hover_text(&val);
            let type_hint = contents.as_deref().and_then(extract_type_hint).map(str::to_string);
            let range     = extract_range(&val);
            Ok(HoverInfoResult {
                kind:      "success".into(),
                path:      params.path,
                line:      params.line,
                column:    params.column,
                contents,
                type_hint,
                range,
                source:    "lsp".into(),
            })
        }
        None => Ok(HoverInfoResult {
            kind:      "no_info".into(),
            path:      params.path,
            line:      params.line,
            column:    params.column,
            contents:  None,
            type_hint: None,
            range:     None,
            source:    "none".into(),
        }),
    }
}

// ─── LSP hover ───────────────────────────────────────────────────────────────

async fn try_lsp_hover(
    path:     &Path,
    content:  &str,
    language: &str,
    line:     usize,
    column:   usize,
    state:    &AppState,
) -> Option<serde_json::Value> {
    if language == "unknown" { return None; }

    let timeout = Duration::from_secs(8);
    let pool    = state.lsp_pool.clone();
    let path_b  = path.to_path_buf();
    let content_b = content.to_string();
    let lang_b  = language.to_string();
    let ws      = state.workspace.as_ref().to_path_buf();

    let result = tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
        let mut pool_lock = pool.lock().unwrap();
        let session = pool_lock
            .get_or_start(&lang_b, &ws)
            .ok()?;
        session.request_hover(&path_b, &lang_b, &content_b, line, column).ok()?
    })).await;

    match result {
        Ok(Ok(Some(v))) => Some(v),
        _ => None,
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Extract markdown text from LSP hover result.
fn extract_hover_text(val: &serde_json::Value) -> Option<String> {
    // contents can be: string | { kind, value } | { language, value } | array of above
    let contents = val.get("contents")?;
    if let Some(s) = contents.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = contents.as_object() {
        if let Some(v) = obj.get("value").and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    if let Some(arr) = contents.as_array() {
        let parts: Vec<String> = arr.iter().filter_map(|item| {
            if let Some(s) = item.as_str() {
                Some(s.to_string())
            } else if let Some(v) = item.get("value").and_then(|v| v.as_str()) {
                Some(v.to_string())
            } else {
                None
            }
        }).collect();
        if !parts.is_empty() {
            return Some(parts.join("\n\n"));
        }
    }
    None
}

/// Try to extract a type signature from hover markdown (first code block or line).
fn extract_type_hint(text: &str) -> Option<&str> {
    // Grab content of first fenced code block
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        // Skip language identifier line
        if let Some(nl) = after.find('\n') {
            let code_start = nl + 1;
            if let Some(end) = after[code_start..].find("```") {
                let snippet = after[code_start..code_start + end].trim();
                if !snippet.is_empty() {
                    return Some(snippet);
                }
            }
        }
    }
    // Fallback: first non-empty line
    text.lines().find(|l| !l.trim().is_empty())
}

/// Extract hover range from LSP result.
fn extract_range(val: &serde_json::Value) -> Option<HoverRange> {
    let range = val.get("range")?;
    let start = range.get("start")?;
    let end   = range.get("end")?;
    Some(HoverRange {
        start_line: start.get("line")?.as_u64()? as usize + 1,
        start_col:  start.get("character")?.as_u64()? as usize + 1,
        end_line:   end.get("line")?.as_u64()? as usize + 1,
        end_col:    end.get("character")?.as_u64()? as usize + 1,
    })
}
