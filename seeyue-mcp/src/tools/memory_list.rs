// src/tools/memory_list.rs
//
// List persisted memory entries from .ai/memory/index.json.
// Supports optional tag filter and limit.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::tools::memory_write::{MemoryIndex, MemoryIndexEntry};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MemoryListParams {
    /// Filter by tag (exact match on any tag in the entry).
    pub tag:   Option<String>,
    /// Maximum entries to return (default: 50, max: 200).
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct MemoryListEntry {
    pub key:     String,
    pub tags:    Vec<String>,
    pub updated: String,
    pub preview: String,
}

#[derive(Debug, Serialize)]
pub struct MemoryListResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success" | "empty"
    pub total:     usize,
    pub truncated: bool,
    pub entries:   Vec<MemoryListEntry>,
}

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT:     usize = 200;

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_memory_list(
    params: MemoryListParams,
    workspace: &Path,
) -> Result<MemoryListResult, ToolError> {
    let index_path = workspace.join(".ai/memory/index.json");

    if !index_path.exists() {
        return Ok(MemoryListResult {
            kind:      "empty".into(),
            total:     0,
            truncated: false,
            entries:   vec![],
        });
    }

    let raw = fs::read_to_string(&index_path)
        .map_err(|e| ToolError::IoError { message: format!("read index: {e}") })?;
    let index: MemoryIndex = serde_json::from_str(&raw).unwrap_or_default();

    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let mut entries: Vec<(String, MemoryIndexEntry)> = index
        .into_iter()
        .filter(|(_, entry)| {
            if let Some(ref t) = params.tag {
                entry.tags.iter().any(|tag| tag.eq_ignore_ascii_case(t))
            } else {
                true
            }
        })
        .collect();

    // Sort by updated descending
    entries.sort_by(|a, b| b.1.updated.cmp(&a.1.updated));

    let total     = entries.len();
    let truncated = total > limit;
    entries.truncate(limit);

    let entries = entries
        .into_iter()
        .map(|(key, entry)| MemoryListEntry {
            key,
            tags:    entry.tags,
            updated: entry.updated,
            preview: entry.preview,
        })
        .collect();

    Ok(MemoryListResult {
        kind: "success".into(),
        total,
        truncated,
        entries,
    })
}
