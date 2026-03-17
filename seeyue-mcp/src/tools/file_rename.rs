// src/tools/file_rename.rs
//
// Atomically rename/move a file within the workspace and record a checkpoint.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::ToolError;
use crate::storage::checkpoint::CheckpointStore;
use crate::tools::read::resolve_path;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FileRenameParams {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Serialize)]
pub struct FileRenameResult {
    #[serde(rename = "type")]
    pub kind:          String, // "success"
    pub old_path:      String,
    pub new_path:      String,
    pub checkpoint_id: String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_file_rename(
    params: FileRenameParams,
    checkpoint: &CheckpointStore,
    workspace: &Path,
) -> Result<FileRenameResult, ToolError> {
    let src = resolve_path(workspace, &params.old_path)?;
    let dst = resolve_path(workspace, &params.new_path)?;

    if !src.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.old_path.clone(),
            hint: "Source file does not exist.".into(),
        });
    }
    if dst.exists() {
        return Err(ToolError::IoError {
            message: format!("Destination already exists: {}", params.new_path),
        });
    }

    // Create parent directory if needed
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ToolError::IoError { message: format!("Cannot create dir: {e}") })?;
    }

    // Capture checkpoint of source file before rename
    let captured_at = chrono::Utc::now().timestamp_millis();
    checkpoint
        .capture(&src, "file_rename", "file_rename")
        .map_err(|e| ToolError::IoError { message: format!("Checkpoint failed: {e:?}") })?;

    // Atomic rename
    std::fs::rename(&src, &dst)
        .map_err(|e| ToolError::IoError { message: format!("Rename failed: {e}") })?;

    Ok(FileRenameResult {
        kind:          "success".into(),
        old_path:      params.old_path,
        new_path:      params.new_path,
        checkpoint_id: captured_at.to_string(),
    })
}
