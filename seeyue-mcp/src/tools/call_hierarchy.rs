// src/tools/call_hierarchy.rs
//
// Static call-hierarchy analysis: find callers or callees of a named symbol
// using regex-based search across the workspace.
// No LSP required — works offline with any supported language.

use serde::{Deserialize, Serialize};
use std::path::Path;

use ignore::WalkBuilder;
use regex::Regex;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::treesitter::languages;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CallHierarchyParams {
    /// Symbol name to analyse (function / method name)
    pub symbol:    String,
    /// "callers" | "callees" | "both" (default: "callers")
    pub direction: Option<String>,
    /// Max results (default: 50)
    pub limit:     Option<usize>,
    /// Restrict search to this relative sub-path (optional)
    pub path:      Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CallSite {
    pub path:    String,
    pub line:    usize,
    pub preview: String,
    pub kind:    String, // "caller" | "callee"
}

#[derive(Debug, Serialize)]
pub struct CallHierarchyResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success"
    pub symbol:    String,
    pub direction: String,
    pub total:     usize,
    pub truncated: bool,
    pub sites:     Vec<CallSite>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT:     usize = 200;

pub fn run_call_hierarchy(
    params: CallHierarchyParams,
    workspace: &Path,
) -> Result<CallHierarchyResult, ToolError> {
    if params.symbol.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "symbol".into(),
            hint: "Provide a non-empty symbol name.".into(),
        });
    }

    let direction = params.direction.as_deref().unwrap_or("callers").to_string();
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    // Determine search root
    let search_root = if let Some(ref p) = params.path {
        let abs = workspace.join(p);
        let canon_ws = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
        let canon_p  = abs.canonicalize().unwrap_or(abs.clone());
        if !canon_p.starts_with(&canon_ws) {
            return Err(ToolError::PathEscape {
                file_path: p.clone(),
                hint: "Path must be inside the workspace.".into(),
            });
        }
        abs
    } else {
        workspace.to_path_buf()
    };

    let sym = &params.symbol;

    // Caller pattern: symbol_name( — call sites (word boundary before symbol)
    let caller_re = Regex::new(&format!(r"\b{}\s*\(", regex::escape(sym)))
        .map_err(|e| ToolError::IoError { message: format!("Regex error: {e}") })?;

    // Callee pattern: fn/def/func symbol_name — definitions
    let callee_re = Regex::new(&format!(
        r"(?:fn|def|func|function|async fn|async def)\s+{}",
        regex::escape(sym)
    )).map_err(|e| ToolError::IoError { message: format!("Regex error: {e}") })?;

    let mut sites: Vec<CallSite> = Vec::new();
    let mut total_found = 0usize;

    let walker = WalkBuilder::new(&search_root)
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .build();

    'outer: for entry in walker {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let path = entry.path();
        if !path.is_file() { continue; }

        let lang = languages::detect_language(path);
        if !matches!(lang.as_str(),
            "rust" | "python" | "javascript" | "typescript" | "tsx" | "go"
        ) {
            continue;
        }

        let data = match safe_read(path) { Ok(d) => d, Err(_) => continue };
        let rel = path.strip_prefix(workspace).unwrap_or(path)
            .to_string_lossy().replace('\\', "/");

        for (i, line) in data.content.lines().enumerate() {
            let lineno = i + 1;
            let is_caller = caller_re.is_match(line);
            let is_callee = callee_re.is_match(line);

            let kind = match direction.as_str() {
                "callers" if is_caller && !is_callee => "caller",
                "callees" if is_callee              => "callee",
                "both" if is_caller && !is_callee   => "caller",
                "both" if is_callee                 => "callee",
                _ => continue,
            };

            total_found += 1;
            if sites.len() < limit {
                sites.push(CallSite {
                    path:    rel.clone(),
                    line:    lineno,
                    preview: line.trim().to_string(),
                    kind:    kind.into(),
                });
            }

            if total_found >= MAX_LIMIT * 2 { break 'outer; }
        }
    }

    let truncated = total_found > limit;

    Ok(CallHierarchyResult {
        kind: "success".into(),
        symbol: params.symbol,
        direction,
        total: total_found,
        truncated,
        sites,
    })
}
