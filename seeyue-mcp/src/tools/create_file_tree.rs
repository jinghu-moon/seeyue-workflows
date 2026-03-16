// src/tools/create_file_tree.rs
//
// Scaffold batch file/directory creation.
// Creates parent directories automatically.
// overwrite=false (default): existing files are skipped and reported.
// One checkpoint group per call (rewindable for created files).

use std::path::Path;

use serde::Serialize;

use crate::checkpoint::CheckpointStore;
use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct FileNode {
    pub path:     String,           // relative to base_path
    pub content:  Option<String>,   // file content; None = create empty file
    #[allow(dead_code)]
    pub template: Option<String>,   // template name (future use, currently unused)
}

pub struct CreateFileTreeParams {
    pub base_path:  String,
    pub tree:       Vec<FileNode>,
    pub overwrite:  Option<bool>,   // default false
}

#[derive(Debug, Serialize)]
pub struct CreateFileTreeResult {
    pub status:          String,   // "ok"
    pub created:         usize,
    pub skipped:         usize,
    pub results:         Vec<FileNodeOutcome>,
    pub checkpoint_id:   String,
}

#[derive(Debug, Serialize)]
pub struct FileNodeOutcome {
    pub path:   String,
    pub action: String,   // "created" | "skipped" | "overwritten"
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_create_file_tree(
    params:     CreateFileTreeParams,
    checkpoint: &CheckpointStore,
    workspace:  &Path,
) -> Result<CreateFileTreeResult, ToolError> {
    if params.tree.is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "tree".to_string(),
            hint:    "Provide at least one FileNode.".to_string(),
        });
    }

    let overwrite  = params.overwrite.unwrap_or(false);
    let call_id    = format!("create_file_tree_{}", chrono::Utc::now().timestamp_millis());

    // Resolve base_path
    let base_abs = crate::platform::path::resolve(workspace, &params.base_path)
        .map_err(|e| ToolError::PathEscape {
            file_path: params.base_path.clone(),
            hint:      format!("{:?}", e),
        })?;

    let mut results: Vec<FileNodeOutcome> = Vec::new();
    let mut created = 0usize;
    let mut skipped = 0usize;

    for node in &params.tree {
        // Resolve each node path relative to base_abs
        let node_abs = crate::platform::path::resolve(&base_abs, &node.path)
            .map_err(|e| ToolError::PathEscape {
                file_path: node.path.clone(),
                hint:      format!("{:?}", e),
            })?;

        // Detect if this is a directory node (path ends with /)
        let is_dir = node.path.ends_with('/') || node.path.ends_with('\\');

        if is_dir {
            // Create directory
            std::fs::create_dir_all(&node_abs).map_err(|e| ToolError::IoError {
                message: format!("Cannot create directory {}: {}", node.path, e),
            })?;
            results.push(FileNodeOutcome {
                path:   node.path.clone(),
                action: "created".to_string(),
            });
            created += 1;
            continue;
        }

        // Ensure parent directories exist
        if let Some(parent) = node_abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::IoError {
                message: format!("Cannot create parent dir for {}: {}", node.path, e),
            })?;
        }

        if node_abs.exists() && !overwrite {
            results.push(FileNodeOutcome {
                path:   node.path.clone(),
                action: "skipped".to_string(),
            });
            skipped += 1;
            continue;
        }

        // Checkpoint existing file before overwrite
        if node_abs.exists() {
            let _ = checkpoint.capture(&node_abs, &call_id, "create_file_tree");
        }

        let content = node.content.as_deref().unwrap_or("");
        std::fs::write(&node_abs, content).map_err(|e| ToolError::IoError {
            message: format!("Cannot write {}: {}", node.path, e),
        })?;

        let action = if node_abs.exists() && overwrite { "overwritten" } else { "created" };
        results.push(FileNodeOutcome {
            path:   node.path.clone(),
            action: action.to_string(),
        });
        created += 1;
    }

    Ok(CreateFileTreeResult {
        status:       "ok".to_string(),
        created,
        skipped,
        results,
        checkpoint_id: call_id,
    })
}
