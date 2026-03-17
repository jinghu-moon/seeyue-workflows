// src/tools/memory_read.rs
//
// Read/search persisted memory entries from .ai/memory/.
// Searches index.json by query (matches key, tags, preview).
// Returns content inline when only one entry matches.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::tools::memory_write::{MemoryIndex, MemoryIndexEntry};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MemoryReadParams {
    /// Free-text query matched against key, tags, and preview.
    pub query: String,
    /// Filter by tag (exact match on any tag in the entry).
    pub tag:   Option<String>,
    /// Maximum entries to return (default: 10, max: 50).
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct MemoryEntry {
    pub key:     String,
    pub tags:    Vec<String>,
    pub updated: String,
    pub preview: String,
    /// Full content — only populated when exactly one entry matches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MemoryReadResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success" | "empty"
    pub query:     String,
    pub total:     usize,
    pub truncated: bool,
    pub entries:   Vec<MemoryEntry>,
}

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT:     usize = 50;

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_memory_read(
    params: MemoryReadParams,
    workspace: &Path,
) -> Result<MemoryReadResult, ToolError> {
    if params.query.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "query".into(),
            hint: "Provide a search query to match against memory keys, tags, or content.".into(),
        });
    }

    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let index_path = workspace.join(".ai/memory/index.json");

    if !index_path.exists() {
        return Ok(MemoryReadResult {
            kind:      "empty".into(),
            query:     params.query,
            total:     0,
            truncated: false,
            entries:   vec![],
        });
    }

    let raw = fs::read_to_string(&index_path)
        .map_err(|e| ToolError::IoError { message: format!("read index: {e}") })?;
    let index: MemoryIndex = serde_json::from_str(&raw).unwrap_or_default();

    let query_lower = params.query.to_lowercase();

    // Filter: match query against key + tags + preview; optional tag filter
    let mut matched: Vec<(String, MemoryIndexEntry)> = index
        .into_iter()
        .filter(|(key, entry)| {
            // Tag filter
            if let Some(ref t) = params.tag {
                if !entry.tags.iter().any(|tag| tag.eq_ignore_ascii_case(t)) {
                    return false;
                }
            }
            // Free-text match
            key.to_lowercase().contains(&query_lower)
                || entry.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
                || entry.preview.to_lowercase().contains(&query_lower)
        })
        .collect();

    // Sort by updated descending
    matched.sort_by(|a, b| b.1.updated.cmp(&a.1.updated));

    let total = matched.len();
    let truncated = total > limit;
    matched.truncate(limit);

    // If exactly one result, load full content
    let load_content = total == 1;

    let entries = matched
        .into_iter()
        .map(|(key, entry)| {
            let content = if load_content {
                let file_path = workspace.join(".ai/memory").join(format!("{key}.md"));
                fs::read_to_string(&file_path).ok()
            } else {
                None
            };
            MemoryEntry {
                key,
                tags:    entry.tags,
                updated: entry.updated,
                preview: entry.preview,
                content,
            }
        })
        .collect();

    Ok(MemoryReadResult {
        kind:      "success".into(),
        query:     params.query,
        total,
        truncated,
        entries,
    })
}
