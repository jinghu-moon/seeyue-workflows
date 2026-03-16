use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::lsp;
use crate::tools::read::resolve_path;
use crate::tools::search_workspace;
use crate::treesitter::languages;
use crate::AppState;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FindDefinitionParams {
    pub path:   String,
    pub line:   usize,
    pub column: usize,
}

#[derive(Debug, Serialize)]
pub struct DefinitionLocation {
    pub path:   String,
    pub line:   usize,
    pub column: usize,
    pub preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FindDefinitionResult {
    #[serde(rename = "type")]
    pub kind:        String, // "success"
    pub symbol:      String,
    pub definitions: Vec<DefinitionLocation>,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub async fn run_find_definition(
    params: FindDefinitionParams,
    state: &AppState,
) -> Result<FindDefinitionResult, ToolError> {
    let path = resolve_path(&state.workspace, &params.path)?;
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let content = file_data.content;
    let language = languages::detect_language(&path);
    let symbol = extract_symbol_at(&content, params.line, params.column)
        .unwrap_or_else(|| "".into());

    let lsp_result = try_lsp_definition(&path, &content, &language, params.line, params.column, state).await;
    let mut definitions = match lsp_result {
        Ok(list) if !list.is_empty() => {
            list.into_iter().filter_map(|loc| {
                let preview = read_preview_line(&loc.path, loc.line).unwrap_or_default();
                Some(DefinitionLocation {
                    path: loc.path.to_string_lossy().to_string(),
                    line: loc.line,
                    column: loc.column,
                    preview,
                    source: Some("lsp".into()),
                })
            }).collect()
        }
        _ => fallback_grep(&symbol, &language, state),
    };

    // 如果 LSP 没有结果且 grep 也为空，保底返回空列表
    if definitions.is_empty() {
        definitions = Vec::new();
    }

    Ok(FindDefinitionResult {
        kind: "success".into(),
        symbol,
        definitions,
    })
}

// ─── LSP ────────────────────────────────────────────────────────────────────

async fn try_lsp_definition(
    path: &Path,
    content: &str,
    language: &str,
    line: usize,
    column: usize,
    state: &AppState,
) -> Result<Vec<lsp::LspLocation>, ToolError> {
    let pool = state.lsp_pool.clone();
    let workspace = state.workspace.clone();
    let path_buf = PathBuf::from(path);
    let content = content.to_string();
    let language = language.to_string();
    let language_for_task = language.clone();

    let task = tokio::task::spawn_blocking(move || {
        let mut pool = pool.lock().map_err(|_| ToolError::LspError {
            message: "LSP pool lock poisoned".into(),
            hint: "Retry the request.".into(),
        })?;
        let session = pool.get_or_start(&language_for_task, &workspace)?;
        let lang_id = lsp::language_id(&language_for_task);
        session.request_definition(&path_buf, lang_id, &content, line, column)
    });

    match tokio::time::timeout(Duration::from_secs(3), task).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(ToolError::LspError {
            message: e.to_string(),
            hint: "LSP task join failed.".into(),
        }),
        Err(_) => Err(ToolError::LspTimeout {
            language: language.to_string(),
            hint: "LSP request exceeded 3s timeout. Falling back to grep.".into(),
        }),
    }
}

// ─── Grep fallback ──────────────────────────────────────────────────────────

fn fallback_grep(symbol: &str, language: &str, state: &AppState) -> Vec<DefinitionLocation> {
    if symbol.trim().is_empty() {
        return Vec::new();
    }

    let ext = extension_for_language(language);
    let glob = ext.map(|e| format!("**/*.{}", e));

    let result = search_workspace::run_search_workspace(
        search_workspace::SearchWorkspaceParams {
            pattern: symbol.to_string(),
            is_regex: Some(false),
            file_glob: glob,
            context_lines: Some(0),
            max_results: Some(50),
        },
        &state.workspace,
    );

    match result {
        Ok(r) => r.matches.into_iter().map(|m| DefinitionLocation {
            path: m.path,
            line: m.line,
            column: m.column,
            preview: m.content,
            source: Some("grep".into()),
        }).collect(),
        Err(_) => Vec::new(),
    }
}

fn extension_for_language(language: &str) -> Option<&'static str> {
    match language {
        "rust" => Some("rs"),
        "python" => Some("py"),
        "typescript" => Some("ts"),
        "tsx" => Some("tsx"),
        "javascript" => Some("js"),
        "jsx" => Some("jsx"),
        "go" => Some("go"),
        _ => None,
    }
}

// ─── 预览与符号提取 ──────────────────────────────────────────────────────────

fn read_preview_line(path: &Path, line: usize) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let line_text = content.lines().nth(line.saturating_sub(1)).unwrap_or("");
    Some(line_text.trim_end().to_string())
}

fn extract_symbol_at(content: &str, line: usize, column: usize) -> Option<String> {
    let line_text = content.lines().nth(line.saturating_sub(1))?;
    let bytes = line_text.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut idx = column.saturating_sub(1).min(bytes.len().saturating_sub(1));
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    if !is_ident(bytes[idx]) {
        if idx > 0 && is_ident(bytes[idx - 1]) {
            idx -= 1;
        } else {
            return None;
        }
    }

    let mut start = idx;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = idx + 1;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }

    Some(String::from_utf8_lossy(&bytes[start..end]).to_string())
}
