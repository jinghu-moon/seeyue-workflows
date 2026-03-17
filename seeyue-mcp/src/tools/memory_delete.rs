// src/tools/memory_delete.rs
//
// Delete a named memory entry from .ai/memory/<key>.md.
// Removes the file and the corresponding index.json entry.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::tools::memory_write::{MemoryIndex, validate_key};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MemoryDeleteParams {
    /// Memory key to delete (e.g. "decisions/arch-v4").
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct MemoryDeleteResult {
    #[serde(rename = "type")]
    pub kind: String, // "deleted" | "not_found"
    pub key:  String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_memory_delete(
    params: MemoryDeleteParams,
    workspace: &Path,
) -> Result<MemoryDeleteResult, ToolError> {
    validate_key(&params.key)?;

    let memory_dir  = workspace.join(".ai/memory");
    let file_path   = memory_dir.join(format!("{}.md", params.key));
    let index_path  = memory_dir.join("index.json");

    if !file_path.exists() {
        return Ok(MemoryDeleteResult { kind: "not_found".into(), key: params.key });
    }

    // Remove content file
    fs::remove_file(&file_path)
        .map_err(|e| ToolError::IoError { message: format!("remove memory file: {e}") })?;

    // Update index.json
    if index_path.exists() {
        let raw = fs::read_to_string(&index_path)
            .map_err(|e| ToolError::IoError { message: format!("read index: {e}") })?;
        let mut index: MemoryIndex = serde_json::from_str(&raw).unwrap_or_default();
        index.remove(&params.key);
        let json = serde_json::to_string_pretty(&index)
            .map_err(|e| ToolError::IoError { message: format!("serialize index: {e}") })?;
        fs::write(&index_path, json)
            .map_err(|e| ToolError::IoError { message: format!("write index: {e}") })?;
    }

    Ok(MemoryDeleteResult { kind: "deleted".into(), key: params.key })
}
