// src/tools/symbol_rename_preview.rs
//
// Dry-run preview of a symbol rename across the entire project.
// Uses LSP references to find all usage sites, then computes affected
// files and line counts WITHOUT writing any changes.
//
// Falls back to grep-based search when LSP is unavailable.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::lsp;
use crate::tools::read::resolve_path;
use crate::treesitter::languages;
use crate::app_state::AppState;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct SymbolRenamePreviewParams {
    pub path:     String,
    pub line:     usize,
    pub column:   usize,
    pub new_name: String,
}

#[derive(Debug, Serialize)]
pub struct SymbolRenamePreviewResult {
    pub status:               String,  // "ok" | "LSP_NOT_AVAILABLE"
    pub symbol:               String,
    pub new_name:             String,
    pub source:               String,  // "lsp" | "grep"
    pub affected_files_count: usize,
    pub affected_files:       Vec<AffectedFile>,
    pub total_occurrences:    usize,
    pub dry_run:              bool,
}

#[derive(Debug, Serialize)]
pub struct AffectedFile {
    pub path:        String,
    pub occurrences: usize,
    pub lines:       Vec<u32>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_symbol_rename_preview(
    params: SymbolRenamePreviewParams,
    state: &AppState,
) -> Result<SymbolRenamePreviewResult, ToolError> {
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

    // Extract symbol name at position for the result label
    let symbol = extract_symbol_at(&content, params.line, params.column)
        .unwrap_or_else(|| params.new_name.clone());

    // Try LSP references first
    let lsp_result = {
        let lang_id = lsp::language_id(&language);
        match state.lsp_pool.lock() {
            Ok(mut pool) => {
                match pool.get_or_start(&language, &state.workspace) {
                    Ok(session) => {
                        match session.request_references(
                            &path, lang_id, &content,
                            params.line, params.column,
                        ) {
                            Ok(locations) => Some((locations, "lsp".to_string())),
                            Err(_) => None,
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    };

    let (affected_files, source, status) = if let Some((locations, src)) = lsp_result {
        let files = group_by_file(locations, &state.workspace);
        (files, src, "ok".to_string())
    } else {
        // Grep fallback: search for symbol name as literal pattern
        let grep_results = grep_symbol(&symbol, &state.workspace);
        (grep_results, "grep".to_string(), "LSP_NOT_AVAILABLE".to_string())
    };

    let total_occurrences: usize = affected_files.iter().map(|f| f.occurrences).sum();
    let affected_files_count = affected_files.len();

    Ok(SymbolRenamePreviewResult {
        status,
        symbol,
        new_name: params.new_name,
        source,
        affected_files_count,
        affected_files,
        total_occurrences,
        dry_run: true,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn extract_symbol_at(content: &str, line: usize, column: usize) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = line.saturating_sub(1);
    let col_idx  = column.saturating_sub(1);
    let line_str = lines.get(line_idx)?;
    let chars: Vec<char> = line_str.chars().collect();
    if col_idx >= chars.len() { return None; }
    // Extend left and right while alphanumeric or _
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut start = col_idx;
    while start > 0 && is_ident(chars[start - 1]) { start -= 1; }
    let mut end = col_idx;
    while end < chars.len() && is_ident(chars[end]) { end += 1; }
    if start == end { return None; }
    Some(chars[start..end].iter().collect())
}

fn group_by_file(
    locations: Vec<lsp::LspLocation>,
    workspace: &Path,
) -> Vec<AffectedFile> {
    let mut map: HashMap<String, Vec<u32>> = HashMap::new();
    for loc in locations {
        let rel = loc.path
            .strip_prefix(workspace)
            .unwrap_or(&loc.path)
            .to_string_lossy()
            .replace('\\', "/");
        map.entry(rel).or_default().push(loc.line as u32);
    }
    let mut result: Vec<AffectedFile> = map.into_iter().map(|(path, mut lines)| {
        lines.sort();
        let occurrences = lines.len();
        AffectedFile { path, occurrences, lines }
    }).collect();
    result.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));
    result
}

fn grep_symbol(symbol: &str, workspace: &Path) -> Vec<AffectedFile> {
    // Use ripgrep-style search via search_workspace logic
    // Simple fallback: scan files for literal symbol
    use walkdir::WalkDir;
    let supported = ["rs", "ts", "tsx", "js", "jsx", "py"];
    let mut files: Vec<AffectedFile> = Vec::new();

    for entry in WalkDir::new(workspace)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !n.starts_with('.') && n != "target" && n != "node_modules"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !supported.contains(&ext) { continue; }

        let Ok(content) = std::fs::read_to_string(path) else { continue; };
        let mut hit_lines: Vec<u32> = Vec::new();
        for (i, line) in content.lines().enumerate() {
            // Match whole-word occurrences
            if word_match(line, symbol) {
                hit_lines.push((i + 1) as u32);
            }
        }
        if !hit_lines.is_empty() {
            let rel = path.strip_prefix(workspace).unwrap_or(path)
                .to_string_lossy().replace('\\', "/");
            let occurrences = hit_lines.len();
            files.push(AffectedFile { path: rel, occurrences, lines: hit_lines });
        }
    }
    files.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));
    files
}

/// Check if `symbol` appears as a whole word in `line`.
fn word_match(line: &str, symbol: &str) -> bool {
    let is_boundary = |c: char| !c.is_alphanumeric() && c != '_';
    let mut start = 0;
    while let Some(idx) = line[start..].find(symbol) {
        let abs = start + idx;
        let before_ok = abs == 0 || line[..abs].chars().last().map_or(true, is_boundary);
        let after_ok  = abs + symbol.len() >= line.len()
            || line[abs + symbol.len()..].chars().next().map_or(true, is_boundary);
        if before_ok && after_ok { return true; }
        start = abs + 1;
        if start >= line.len() { break; }
    }
    false
}
