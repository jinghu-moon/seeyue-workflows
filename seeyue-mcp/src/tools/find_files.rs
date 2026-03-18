// src/tools/find_files.rs
//
// find_files: Glob-pattern file search respecting .gitignore.
// Complement to workspace_tree (tree view) and search_workspace (content search).

use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FindFilesParams {
    /// Glob pattern relative to workspace root (e.g. "src/**/*.rs", "**/*.toml").
    pub pattern:           String,
    /// Respect .gitignore and .ignore files (default: true).
    pub respect_gitignore: Option<bool>,
    /// Include hidden files/dirs (default: false).
    pub show_hidden:       Option<bool>,
    /// Maximum results to return (default: 200, max: 1000).
    pub limit:             Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct FileMatch {
    pub path:          String,
    pub size_bytes:    u64,
    pub modified_ago:  String,
}

#[derive(Debug, Serialize)]
pub struct FindFilesResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success" | "empty"
    pub pattern:   String,
    pub total:     usize,
    pub truncated: bool,
    pub files:     Vec<FileMatch>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_find_files(
    params: FindFilesParams,
    workspace: &Path,
) -> Result<FindFilesResult, ToolError> {
    if params.pattern.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "pattern".into(),
            hint:    "Provide a glob pattern e.g. \"src/**/*.rs\".".into(),
        });
    }

    let limit      = params.limit.unwrap_or(200).min(1000);
    let use_ignore = params.respect_gitignore.unwrap_or(true);
    let hidden     = params.show_hidden.unwrap_or(false);

    let glob_built = Glob::new(&params.pattern)
        .map_err(|e| ToolError::InvalidRegex {
            pattern: params.pattern.clone(),
            message: e.to_string(),
            hint:    "Use standard glob syntax: *, **, ?, [abc].".into(),
        })?;
    let mut builder = GlobSetBuilder::new();
    builder.add(glob_built);
    let glob_set: GlobSet = builder.build()
        .map_err(|e| ToolError::InvalidRegex {
            pattern: params.pattern.clone(),
            message: e.to_string(),
            hint:    "Invalid glob pattern.".into(),
        })?;

    let mut files: Vec<FileMatch> = Vec::new();
    let mut truncated = false;

    let walker = ignore::WalkBuilder::new(workspace)
        .standard_filters(use_ignore)
        .hidden(!hidden)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e)  => e,
            Err(_) => continue,
        };
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }

        let rel = match entry.path().strip_prefix(workspace) {
            Ok(r)  => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        if !glob_set.is_match(&rel) {
            continue;
        }

        if files.len() >= limit {
            truncated = true;
            break;
        }

        let meta        = entry.metadata().ok();
        let size_bytes  = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified_ago = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| format_modified_ago(t))
            .unwrap_or_else(|| "?".into());

        files.push(FileMatch { path: rel, size_bytes, modified_ago });
    }

    let total = files.len();
    let kind  = if total == 0 { "empty" } else { "success" };

    Ok(FindFilesResult {
        kind:    kind.into(),
        pattern: params.pattern,
        total,
        truncated,
        files,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn format_modified_ago(t: std::time::SystemTime) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if secs < 60             { format!("{}s", secs) }
    else if secs < 3600      { format!("{}m", secs / 60) }
    else if secs < 86400     { format!("{}h", secs / 3600) }
    else if secs < 86400 * 7 { format!("{}d", secs / 86400) }
    else                     { format!("{}w", secs / (86400 * 7)) }
}
