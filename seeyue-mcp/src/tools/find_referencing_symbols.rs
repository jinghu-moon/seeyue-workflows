// src/tools/find_referencing_symbols.rs
//
// sy_find_referencing_symbols: find all symbols that reference a given symbol.
// Steps:
//   1. sy_find_symbol → get (path, line, col) of the target symbol
//   2. LspSession.request_references() → Vec<LspLocation>
//   3. For each LspLocation, find innermost enclosing symbol (name_path)

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::app_state::AppState;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::find_symbol::{run_find_symbol, FindSymbolParams};
use crate::tools::get_symbols_overview::{run_get_symbols_overview, GetSymbolsOverviewParams, OverviewSymbol};
use crate::treesitter::languages::detect_language;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FindReferencingSymbolsParams {
    /// name_path of the target symbol, e.g. "UserSession/validate"
    pub name_path:     String,
    /// File containing the symbol definition (relative path)
    pub relative_path: String,
}

// ─── Response ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct ReferenceEntry {
    pub name_path:     String, // enclosing symbol or "<file>" or "<macro>"
    pub relative_path: String,
    pub line:          usize,
    pub snippet:       String,
}

#[derive(Debug, Serialize)]
pub struct FindReferencingSymbolsResult {
    #[serde(rename = "type")]
    pub kind:       String, // "success"
    pub references: Vec<ReferenceEntry>,
}

// ─── Main logic ───────────────────────────────────────────────────────────────

pub async fn run_find_referencing_symbols(
    params: FindReferencingSymbolsParams,
    state:  &AppState,
) -> Result<FindReferencingSymbolsResult, ToolError> {
    // 1. Locate the target symbol to get its definition position
    let sym_result = run_find_symbol(
        FindSymbolParams {
            name_path_pattern:  params.name_path.clone(),
            relative_path:      Some(params.relative_path.clone()),
            substring_matching: Some(false),
            include_body:       Some(false),
            depth:              Some(1),
        },
        state,
    ).await?;

    let target = sym_result.matches.into_iter().next().ok_or_else(|| ToolError::FileNotFound {
        file_path: params.name_path.clone(),
        hint: format!("Symbol '{}' not found in '{}'", params.name_path, params.relative_path),
    })?;

    let def_path = state.workspace.join(&target.file);
    let def_content = safe_read(&def_path)?.content;
    let language = detect_language(&def_path);

    // 2. Get LSP references
    let lsp_refs = {
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let mut pool = state.lsp_pool.lock().map_err(|_| ToolError::LspError {
                message: "LSP pool mutex poisoned".into(),
                hint: "Restart the MCP server.".into(),
            })?;
            let session = pool.get_or_start(&language, &state.workspace)?;
            session.request_references(&def_path, &language, &def_content, target.start_line, 1)
        })
        .await
        .map_err(|_| ToolError::LspTimeout {
            language: language.clone(),
            hint: "LSP references request timed out.".into(),
        })??;
        result
    };

    if lsp_refs.is_empty() {
        return Ok(FindReferencingSymbolsResult {
            kind:       "success".into(),
            references: vec![],
        });
    }

    // 3. For each reference location, find enclosing symbol
    let mut references = Vec::new();
    for loc in lsp_refs {
        let rel = loc.path
            .strip_prefix(&*state.workspace)
            .unwrap_or(&loc.path)
            .to_string_lossy()
            .replace('\\', "/");

        // Read enclosing file and get symbol tree
        let enc_content = safe_read(&loc.path).ok().map(|d| d.content);
        let enc_lang    = detect_language(&loc.path);
        let symbols     = if let Some(ref c) = enc_content {
            let overview = run_get_symbols_overview(
                GetSymbolsOverviewParams {
                    relative_path: rel.clone(),
                    depth: Some(2),
                },
                state,
            ).await.unwrap_or_else(|_| crate::tools::get_symbols_overview::GetSymbolsOverviewResult {
                kind: "success".into(),
                source: "syntax".into(),
                symbols: vec![],
            });
            overview.symbols
        } else {
            vec![]
        };

        let name_path = find_enclosing_symbol(&symbols, loc.line, None)
            .unwrap_or_else(|| "<file>".to_string());

        // Extract snippet
        let snippet = enc_content
            .as_deref()
            .and_then(|c| c.lines().nth(loc.line.saturating_sub(1)))
            .unwrap_or("")
            .trim()
            .to_string();

        references.push(ReferenceEntry {
            name_path,
            relative_path: rel,
            line: loc.line,
            snippet,
        });
    }

    Ok(FindReferencingSymbolsResult { kind: "success".into(), references })
}

// ─── Enclosing symbol finder ──────────────────────────────────────────────────

/// Find the innermost symbol enclosing `line` (1-indexed).
/// Returns the name_path string, or None if no symbol encloses the line.
pub fn find_enclosing_symbol(
    symbols:    &[OverviewSymbol],
    line:       usize,
    parent_np:  Option<&str>,
) -> Option<String> {
    let mut best: Option<(usize, String)> = None; // (depth, name_path)

    for sym in symbols {
        if line >= sym.start_line && line <= sym.end_line {
            let np = match parent_np {
                Some(p) => format!("{}/{}", p, sym.name),
                None    => sym.name.clone(),
            };
            // Check children for a deeper match
            let child_match = find_enclosing_symbol(&sym.children, line, Some(&np));
            let result_np = child_match.unwrap_or(np);
            // Track deepest (we replace any existing)
            best = Some((0, result_np));
        }
    }

    best.map(|(_, np)| np)
}
