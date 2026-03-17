// src/tools/snapshot_workspace.rs
//
// Copy the workspace (respecting .gitignore) into a timestamped snapshot
// directory under <workspace>/.seeyue/snapshots/<timestamp>/.
// Useful for cross-session restore or diffing a known-good state.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

use ignore::WalkBuilder;

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SnapshotWorkspaceParams {
    /// Optional label for the snapshot directory (default: unix timestamp ms)
    pub label: Option<String>,
    /// If true, also copy files ignored by .gitignore (default: false)
    pub include_ignored: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotWorkspaceResult {
    #[serde(rename = "type")]
    pub kind:          String, // "success"
    pub snapshot_path: String,
    pub files_copied:  usize,
    pub bytes_copied:  u64,
    pub duration_ms:   u64,
}

// ─── Implementation ──────────────────────────────────────────────────────────

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB per file
const MAX_TOTAL:     u64 = 200 * 1024 * 1024; // 200 MB total

pub fn run_snapshot_workspace(
    params: SnapshotWorkspaceParams,
    workspace: &Path,
) -> Result<SnapshotWorkspaceResult, ToolError> {
    let include_ignored = params.include_ignored.unwrap_or(false);

    let label = params.label.unwrap_or_else(|| {
        chrono::Utc::now().timestamp_millis().to_string()
    });

    // Sanitise label: allow alphanumeric, dash, underscore, dot only
    let label: String = label
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();

    let snapshot_dir = workspace
        .join(".seeyue")
        .join("snapshots")
        .join(&label);

    if snapshot_dir.exists() {
        return Err(ToolError::IoError {
            message: format!("Snapshot already exists: {}", snapshot_dir.display()),
        });
    }

    std::fs::create_dir_all(&snapshot_dir)
        .map_err(|e| ToolError::IoError { message: format!("Cannot create snapshot dir: {e}") })?;

    let start = Instant::now();
    let mut files_copied: usize = 0;
    let mut bytes_copied: u64  = 0;

    let walker = WalkBuilder::new(workspace)
        .hidden(false)
        .ignore(!include_ignored)
        .git_ignore(!include_ignored)
        .git_global(false)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e)  => e,
            Err(_) => continue,
        };

        let src = entry.path();

        // Skip the snapshot dir itself to avoid recursion
        if src.starts_with(&snapshot_dir) {
            continue;
        }
        // Skip .seeyue internal dirs
        if src.starts_with(workspace.join(".seeyue")) {
            continue;
        }

        if !src.is_file() {
            continue;
        }

        // Per-file size guard
        let meta = src.metadata()
            .map_err(|e| ToolError::IoError { message: format!("stat failed: {e}") })?;
        if meta.len() > MAX_FILE_SIZE {
            continue; // skip large files silently
        }
        if bytes_copied + meta.len() > MAX_TOTAL {
            break; // total budget exhausted
        }

        // Compute destination path
        let rel = src.strip_prefix(workspace).unwrap_or(src);
        let dst = snapshot_dir.join(rel);

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolError::IoError { message: format!("mkdir failed: {e}") })?;
        }

        std::fs::copy(src, &dst)
            .map_err(|e| ToolError::IoError { message: format!("copy failed: {e}") })?;

        bytes_copied += meta.len();
        files_copied += 1;
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let snapshot_path = snapshot_dir
        .strip_prefix(workspace)
        .unwrap_or(&snapshot_dir)
        .to_string_lossy()
        .replace('\\', "/");

    Ok(SnapshotWorkspaceResult {
        kind: "success".into(),
        snapshot_path,
        files_copied,
        bytes_copied,
        duration_ms,
    })
}
