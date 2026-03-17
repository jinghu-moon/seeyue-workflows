// src/tools/diff_since_checkpoint.rs
//
// Returns a structured diff of workspace changes relative to the most recent
// SQLite WAL checkpoint snapshot.
// Finer-grained than git_diff_file: includes uncommitted content.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::storage::checkpoint::CheckpointStore;
use crate::render::diff;
use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct DiffSinceCheckpointParams {
    pub label: Option<String>,
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct DiffSinceCheckpointResult {
    pub status:            String,  // "ok" | "NO_CHECKPOINT"
    pub checkpoint_label:  Option<String>,
    pub files:             Vec<FileDiff>,
    pub total_files:       usize,
    pub total_added:       u32,
    pub total_removed:     u32,
}

#[derive(Debug, Serialize)]
pub struct FileDiff {
    pub path:    String,
    pub added:   u32,
    pub removed: u32,
    pub hunks:   Vec<String>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_diff_since_checkpoint(
    params: DiffSinceCheckpointParams,
    workspace: &Path,
    checkpoint_store: &CheckpointStore,
) -> Result<DiffSinceCheckpointResult, ToolError> {
    let snapshots = checkpoint_store.list();

    if snapshots.is_empty() {
        return Ok(DiffSinceCheckpointResult {
            status:           "NO_CHECKPOINT".to_string(),
            checkpoint_label: None,
            files:            Vec::new(),
            total_files:      0,
            total_added:      0,
            total_removed:    0,
        });
    }

    // Pick target snapshot: match label against tool_name or use most recent.
    let target = if let Some(ref label) = params.label {
        snapshots.iter().find(|s| s.tool_name.contains(label.as_str()))
            .or_else(|| snapshots.first())
    } else {
        snapshots.first() // list() returns DESC order → first = most recent
    };

    let snapshot = match target {
        Some(s) => s,
        None => return Ok(DiffSinceCheckpointResult {
            status:           "NO_CHECKPOINT".to_string(),
            checkpoint_label: None,
            files:            Vec::new(),
            total_files:      0,
            total_added:      0,
            total_removed:    0,
        }),
    };

    let checkpoint_label = format!("{} @ {}", snapshot.tool_name, snapshot.captured_at_ms);
    let snap_file_path = PathBuf::from(&snapshot.file_path);

    // Apply path filter
    if let Some(ref filter_paths) = params.paths {
        let rel = snap_file_path.strip_prefix(workspace).unwrap_or(&snap_file_path);
        let rel_str = rel.to_string_lossy();
        if !filter_paths.iter().any(|p| rel_str.contains(p.as_str())) {
            return Ok(DiffSinceCheckpointResult {
                status:           "ok".to_string(),
                checkpoint_label: Some(checkpoint_label),
                files:            Vec::new(),
                total_files:      0,
                total_added:      0,
                total_removed:    0,
            });
        }
    }

    // Read snapshot bytes (the stored pre-write content)
    let snapshot_bytes = checkpoint_store
        .read_snapshot(&snapshot.tool_name, &snap_file_path)
        .unwrap_or_default();
    let snapshot_content = String::from_utf8_lossy(&snapshot_bytes).into_owned();

    // Read current file content
    let current_content = std::fs::read(&snap_file_path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();

    let rel_path = snap_file_path
        .strip_prefix(workspace)
        .unwrap_or(&snap_file_path)
        .to_string_lossy()
        .replace('\\', "/");

    // Compute diff using existing diff module
    let diff_result = diff::compute_diff(&rel_path, &snapshot_content, &current_content, None);

    let added   = diff_result.summary.total_added as u32;
    let removed = diff_result.summary.total_removed as u32;
    let hunks: Vec<String> = diff_result.hunks.iter()
        .map(|h| h.lines.iter()
            .map(|l| format!("{}{}",
                match l.kind {
                    diff::DiffLineKind::Add => "+",
                    diff::DiffLineKind::Del => "-",
                    diff::DiffLineKind::Ctx => " ",
                },
                l.content
            ))
            .collect::<Vec<_>>()
            .join(""))
        .collect();

    let mut files = Vec::new();
    if added > 0 || removed > 0 {
        files.push(FileDiff { path: rel_path, added, removed, hunks });
    }

    let total_files = files.len();

    Ok(DiffSinceCheckpointResult {
        status:           "ok".to_string(),
        checkpoint_label: Some(checkpoint_label),
        files,
        total_files,
        total_added:      added,
        total_removed:    removed,
    })
}
