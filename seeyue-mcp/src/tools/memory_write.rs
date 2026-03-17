// src/tools/memory_write.rs
//
// Persist a named memory entry to .ai/memory/<key>.md.
// Index is maintained in .ai/memory/index.json for fast lookup.
//
// Key format: alphanumeric, dash, underscore, forward-slash (for namespacing).
// Example keys: "decisions/arch-v4", "patterns/rust-error-handling", "incidents/2026-03-17"

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MemoryWriteParams {
    /// Memory key (alphanumeric, dash, underscore, slash). E.g. "decisions/arch-v4".
    pub key:     String,
    /// Markdown content to store.
    pub content: String,
    /// Optional tags for retrieval.
    #[serde(default)]
    pub tags:    Vec<String>,
    /// Write mode: "overwrite" (default) | "append" (append to existing content).
    #[serde(default)]
    pub mode:    Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MemoryWriteResult {
    #[serde(rename = "type")]
    pub kind:    String, // "created" | "updated" | "appended"
    pub key:     String,
    pub path:    String,
    pub tags:    Vec<String>,
    pub updated: String,
}

// ─── Index entry ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryIndexEntry {
    pub tags:    Vec<String>,
    pub updated: String,
    pub preview: String,
}

pub type MemoryIndex = HashMap<String, MemoryIndexEntry>;

// ─── Implementation ──────────────────────────────────────────────────────────

const PREVIEW_CHARS: usize = 200;

pub fn run_memory_write(
    params: MemoryWriteParams,
    workspace: &Path,
) -> Result<MemoryWriteResult, ToolError> {
    validate_key(&params.key)?;

    let memory_dir = workspace.join(".ai/memory");
    fs::create_dir_all(&memory_dir)
        .map_err(|e| ToolError::IoError { message: format!("create .ai/memory: {e}") })?;

    // Write content file
    let rel_path = format!("{}.md", params.key.replace('/', "/"));
    let file_path = memory_dir.join(&rel_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ToolError::IoError { message: format!("create dir: {e}") })?;
    }

    let existed = file_path.exists();
    let mode = params.mode.as_deref().unwrap_or("overwrite");

    let final_content = if mode == "append" && existed {
        let existing = fs::read_to_string(&file_path)
            .map_err(|e| ToolError::IoError { message: format!("read existing memory: {e}") })?;
        format!("{}\n\n---\n\n{}", existing.trim_end(), params.content)
    } else {
        params.content.clone()
    };

    fs::write(&file_path, &final_content)
        .map_err(|e| ToolError::IoError { message: format!("write memory file: {e}") })?;

    let updated = Utc::now().to_rfc3339();
    let preview: String = final_content.chars().take(PREVIEW_CHARS).collect();

    // Update index.json
    let index_path = memory_dir.join("index.json");
    let mut index: MemoryIndex = if index_path.exists() {
        let raw = fs::read_to_string(&index_path)
            .map_err(|e| ToolError::IoError { message: format!("read index: {e}") })?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        HashMap::new()
    };

    index.insert(params.key.clone(), MemoryIndexEntry {
        tags:    params.tags.clone(),
        updated: updated.clone(),
        preview: preview.clone(),
    });

    let index_json = serde_json::to_string_pretty(&index)
        .map_err(|e| ToolError::IoError { message: format!("serialize index: {e}") })?;
    fs::write(&index_path, index_json)
        .map_err(|e| ToolError::IoError { message: format!("write index: {e}") })?;

    Ok(MemoryWriteResult {
        kind:    if !existed { "created" } else if mode == "append" { "appended" } else { "updated" }.into(),
        key:     params.key,
        path:    format!(".ai/memory/{rel_path}"),
        tags:    params.tags,
        updated,
    })
}

/// Validate memory key: alphanumeric, dash, underscore, forward-slash only.
pub fn validate_key(key: &str) -> Result<(), ToolError> {
    if key.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "key".into(),
            hint: "Provide a non-empty key like \"decisions/arch-v4\".".into(),
        });
    }
    if !key.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/')) {
        return Err(ToolError::IoError {
            message: format!(
                "Invalid key '{}': only alphanumeric, dash, underscore, slash allowed.", key
            ),
        });
    }
    if key.starts_with('/') || key.ends_with('/') || key.contains("//") {
        return Err(ToolError::IoError {
            message: format!("Invalid key '{}': cannot start/end with slash or contain '..'.", key),
        });
    }
    Ok(())
}
