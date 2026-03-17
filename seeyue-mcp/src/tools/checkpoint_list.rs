// src/tools/checkpoint_list.rs
//
// List all checkpoint snapshots in the current session.
// Returns file path, tool name, and timestamp per snapshot.

use serde::Serialize;

use crate::error::ToolError;
use crate::storage::checkpoint::CheckpointStore;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct CheckpointListParams {}

#[derive(Debug, Serialize)]
pub struct CheckpointEntry {
    pub file_path:   String,
    pub tool_name:   String,
    pub captured_at: String, // ISO 8601 from epoch ms
}

#[derive(Debug, Serialize)]
pub struct CheckpointListResult {
    #[serde(rename = "type")]
    pub kind:        String, // "success" | "empty"
    pub total:       usize,
    pub checkpoints: Vec<CheckpointEntry>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_checkpoint_list(
    _params: CheckpointListParams,
    store: &CheckpointStore,
) -> Result<CheckpointListResult, ToolError> {
    let snapshots = store.list();
    let total = snapshots.len();

    let checkpoints = snapshots
        .into_iter()
        .map(|s| {
            // Convert epoch ms to ISO 8601
            let captured_at = chrono::DateTime::from_timestamp_millis(s.captured_at_ms)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| s.captured_at_ms.to_string());
            CheckpointEntry {
                file_path:   s.file_path,
                tool_name:   s.tool_name,
                captured_at,
            }
        })
        .collect();

    Ok(CheckpointListResult {
        kind: if total == 0 { "empty" } else { "success" }.into(),
        total,
        checkpoints,
    })
}
