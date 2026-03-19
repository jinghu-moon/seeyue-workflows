// src/tools/get_symbols_overview.rs
//
// sy_get_symbols_overview: return a symbol tree for a file.
// Primary path: LSP textDocument/documentSymbol.
// Fallback path: tree-sitter extract_ts_symbols (source = "syntax").

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::app_state::AppState;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::lsp::LspSymbol;
use crate::treesitter::languages::detect_language;
use crate::treesitter::symbols::{extract_ts_symbols, TsSymbol};
use crate::tools::read::resolve_path;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GetSymbolsOverviewParams {
    pub relative_path: String,
    /// Maximum nesting depth to return. 0 = top-level only, 1 = top + children.
    pub depth: Option<u8>,
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct OverviewSymbol {
    pub name:       String,
    pub kind:       String,
    pub start_line: usize,
    pub end_line:   usize,
    pub children:   Vec<OverviewSymbol>,
}

#[derive(Debug, Serialize)]
pub struct GetSymbolsOverviewResult {
    #[serde(rename = "type")]
    pub kind:    String, // "success"
    pub source:  String, // "lsp" | "syntax"
    pub symbols: Vec<OverviewSymbol>,
}

// ─── Main logic ───────────────────────────────────────────────────────────────

pub async fn run_get_symbols_overview(
    params: GetSymbolsOverviewParams,
    state: &AppState,
) -> Result<GetSymbolsOverviewResult, ToolError> {
    let path = resolve_path(&state.workspace, &params.relative_path)?;
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.relative_path.clone(),
            hint: format!("File '{}' does not exist in workspace.", params.relative_path),
        });
    }

    let file_data = safe_read(&path)?;
    let content   = file_data.content;
    let language  = detect_language(&path);
    let depth     = params.depth.unwrap_or(0);

    // Try LSP first; fall back to tree-sitter on any error.
    let lsp_result = try_lsp_document_symbols(&path, &content, &language, state).await;

    match lsp_result {
        Ok(lsp_syms) if !lsp_syms.is_empty() => {
            let symbols = lsp_syms
                .into_iter()
                .map(|s| lsp_symbol_to_overview(s, depth, 0))
                .collect();
            Ok(GetSymbolsOverviewResult {
                kind:    "success".into(),
                source:  "lsp".into(),
                symbols,
            })
        }
        _ => {
            // Fallback: tree-sitter
            let ts_syms = extract_ts_symbols(&content, &language);
            let symbols = ts_syms
                .into_iter()
                .map(|s| ts_symbol_to_overview(s, depth, 0))
                .collect();
            Ok(GetSymbolsOverviewResult {
                kind:    "success".into(),
                source:  "syntax".into(),
                symbols,
            })
        }
    }
}

// ─── LSP helper ───────────────────────────────────────────────────────────────

async fn try_lsp_document_symbols(
    path: &std::path::Path,
    content: &str,
    language: &str,
    state: &AppState,
) -> Result<Vec<LspSymbol>, ToolError> {
    // Acquire the pool lock synchronously (no async in LspSessionPool).
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut pool = state.lsp_pool.lock().map_err(|_| ToolError::LspError {
            message: "LSP pool mutex poisoned".into(),
            hint: "Restart the MCP server.".into(),
        })?;
        let session = pool.get_or_start(language, &state.workspace)?;
        session.request_document_symbols(path, language, content)
    })
    .await
    .map_err(|_| ToolError::LspTimeout {
        language: language.to_string(),
        hint: "LSP document symbol request timed out; falling back to syntax.".into(),
    })??;

    Ok(result)
}

// ─── Conversion helpers ───────────────────────────────────────────────────────

fn lsp_symbol_to_overview(sym: LspSymbol, max_depth: u8, current_depth: u8) -> OverviewSymbol {
    let children = if current_depth < max_depth {
        sym.children
            .into_iter()
            .map(|c| lsp_symbol_to_overview(c, max_depth, current_depth + 1))
            .collect()
    } else {
        vec![]
    };
    OverviewSymbol {
        name:       sym.name,
        kind:       format!("{:?}", sym.kind).to_lowercase(),
        start_line: sym.start_line,
        end_line:   sym.end_line,
        children,
    }
}

fn ts_symbol_to_overview(sym: TsSymbol, max_depth: u8, current_depth: u8) -> OverviewSymbol {
    let children = if current_depth < max_depth {
        sym.children
            .into_iter()
            .map(|c| ts_symbol_to_overview(c, max_depth, current_depth + 1))
            .collect()
    } else {
        vec![]
    };
    OverviewSymbol {
        name:       sym.name,
        kind:       sym.kind,
        start_line: sym.start_line,
        end_line:   sym.end_line,
        children,
    }
}
