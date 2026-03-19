// src/tools/find_symbol.rs
//
// sy_find_symbol: locate symbols by name_path pattern.
// Builds an in-memory symbol index from get_symbols_overview, then matches.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::app_state::AppState;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::get_symbols_overview::{run_get_symbols_overview, GetSymbolsOverviewParams, OverviewSymbol};
use crate::treesitter::languages::detect_language;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FindSymbolParams {
    /// Name or name_path pattern to search for (e.g. "validate" or "UserSession/validate").
    pub name_path_pattern: String,
    /// Restrict search to this file (relative path). None = search all source files.
    pub relative_path: Option<String>,
    /// If true, match by substring; if false, require exact name equality.
    pub substring_matching: Option<bool>,
    /// If true, attach the symbol's source lines to the result.
    pub include_body: Option<bool>,
    /// Depth passed to get_symbols_overview (controls child visibility).
    pub depth: Option<u8>,
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct SymbolMatch {
    pub name:       String,
    pub name_path:  String,
    pub kind:       String,
    pub start_line: usize,
    pub end_line:   usize,
    pub file:       String,
    pub source:     String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body:       Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FindSymbolResult {
    #[serde(rename = "type")]
    pub kind:    String, // "success"
    pub matches: Vec<SymbolMatch>,
}

// ─── Main logic ───────────────────────────────────────────────────────────────

pub async fn run_find_symbol(
    params: FindSymbolParams,
    state: &AppState,
) -> Result<FindSymbolResult, ToolError> {
    let substring = params.substring_matching.unwrap_or(false);
    let include_body = params.include_body.unwrap_or(false);
    let depth = params.depth.unwrap_or(1);
    let pattern = &params.name_path_pattern;

    let files: Vec<String> = match &params.relative_path {
        Some(p) => vec![p.clone()],
        None    => collect_source_files(&state.workspace),
    };

    let mut matches: Vec<SymbolMatch> = Vec::new();

    for file in files {
        let overview = run_get_symbols_overview(
            GetSymbolsOverviewParams { relative_path: file.clone(), depth: Some(depth) },
            state,
        ).await;

        let overview = match overview {
            Ok(o)  => o,
            Err(_) => continue, // skip unreadable files
        };

        let source_label = overview.source.clone();

        // Read file content once for body extraction
        let content_opt: Option<String> = if include_body {
            let path = state.workspace.join(&file);
            safe_read(&path).ok().map(|d| d.content)
        } else {
            None
        };

        collect_matches(
            &overview.symbols,
            pattern,
            substring,
            include_body,
            &file,
            &source_label,
            content_opt.as_deref(),
            None,
            &mut matches,
        );
    }

    Ok(FindSymbolResult { kind: "success".into(), matches })
}

// ─── Recursive match collector ────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn collect_matches(
    symbols:      &[OverviewSymbol],
    pattern:      &str,
    substring:    bool,
    include_body: bool,
    file:         &str,
    source:       &str,
    content:      Option<&str>,
    parent_path:  Option<&str>,
    out:          &mut Vec<SymbolMatch>,
) {
    for sym in symbols {
        let np = match parent_path {
            Some(p) => format!("{}/{}", p, sym.name),
            None    => sym.name.clone(),
        };

        let is_match = if substring {
            np.contains(pattern.as_ref() as &str)
                || sym.name.contains(pattern.as_ref() as &str)
        } else {
            sym.name == *pattern || np == *pattern
        };

        if is_match {
            let body = if include_body {
                content.map(|c| extract_lines(c, sym.start_line, sym.end_line))
            } else {
                None
            };
            out.push(SymbolMatch {
                name:       sym.name.clone(),
                name_path:  np.clone(),
                kind:       sym.kind.clone(),
                start_line: sym.start_line,
                end_line:   sym.end_line,
                file:       file.to_string(),
                source:     source.to_string(),
                body,
            });
        }

        // Recurse into children
        collect_matches(
            &sym.children,
            pattern,
            substring,
            include_body,
            file,
            source,
            content,
            Some(&np),
            out,
        );
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Extract lines start_line..=end_line (1-indexed) from content.
fn extract_lines(content: &str, start_line: usize, end_line: usize) -> String {
    content
        .lines()
        .enumerate()
        .filter(|(i, _)| {
            let line_no = i + 1;
            line_no >= start_line && line_no <= end_line
        })
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Walk the workspace root and collect relative paths to source files.
fn collect_source_files(workspace: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_source_files_rec(workspace, workspace, &mut files);
    files
}

fn collect_source_files_rec(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e)  => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip hidden dirs and common build dirs
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_source_files_rec(root, &path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "rs" | "py" | "ts" | "tsx" | "js" | "go" | "java" | "c" | "cpp" | "cs" => {
                    if let Ok(rel) = path.strip_prefix(root) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
                _ => {}
            }
        }
    }
}
