// src/tools/batch_read.rs
//
// Read multiple files in a single request, reducing round-trips for agents.
// Files are read in parallel using tokio::task::JoinSet.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::read::resolve_path;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BatchReadParams {
    /// List of relative file paths to read (max 20)
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FileReadEntry {
    pub path:    String,
    pub content: String,
    pub size:    usize,
    pub error:   Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchReadResult {
    #[serde(rename = "type")]
    pub kind:       String, // "success"
    pub total:      usize,
    pub files:      Vec<FileReadEntry>,
    #[serde(default)]
    pub elapsed_ms: u64,
}

// ─── Implementation ──────────────────────────────────────────────────────────

const MAX_PATHS: usize = 20;

pub async fn run_batch_read(
    params: BatchReadParams,
    workspace: &Path,
) -> Result<BatchReadResult, ToolError> {
    if params.paths.is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "paths".into(),
            hint: "Provide at least one file path.".into(),
        });
    }
    if params.paths.len() > MAX_PATHS {
        return Err(ToolError::MissingParameter {
            missing: "paths".into(),
            hint: format!("Maximum {MAX_PATHS} paths per request, got {}.", params.paths.len()),
        });
    }

    let t0 = Instant::now();
    let workspace = workspace.to_path_buf();

    // Resolve paths upfront (sync, cheap).
    let resolved: Vec<(String, Option<PathBuf>)> = params
        .paths
        .iter()
        .map(|p| {
            let abs = resolve_path(&workspace, p).ok();
            (p.clone(), abs)
        })
        .collect();

    // Spawn one async task per file; each task performs blocking IO via spawn_blocking.
    let mut set: tokio::task::JoinSet<FileReadEntry> = tokio::task::JoinSet::new();
    for (orig, abs_opt) in resolved {
        match abs_opt {
            None => {
                set.spawn(async move {
                    FileReadEntry {
                        path:    orig,
                        content: String::new(),
                        size:    0,
                        error:   Some("path resolve error".into()),
                    }
                });
            }
            Some(abs) => {
                set.spawn(async move {
                    match tokio::task::spawn_blocking(move || safe_read(&abs)).await {
                        Ok(Ok(data)) => {
                            let size = data.content.len();
                            FileReadEntry { path: orig, content: data.content, size, error: None }
                        }
                        Ok(Err(e)) => FileReadEntry {
                            path: orig, content: String::new(), size: 0,
                            error: Some(format!("{e:?}")),
                        },
                        Err(e) => FileReadEntry {
                            path: orig, content: String::new(), size: 0,
                            error: Some(format!("spawn error: {e}")),
                        },
                    }
                });
            }
        }
    }

    let mut files: Vec<FileReadEntry> = Vec::with_capacity(params.paths.len());
    while let Some(res) = set.join_next().await {
        match res {
            Ok(entry) => files.push(entry),
            Err(e) => files.push(FileReadEntry {
                path:    String::new(),
                content: String::new(),
                size:    0,
                error:   Some(format!("task panic: {e}")),
            }),
        }
    }

    let total = files.len();
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    Ok(BatchReadResult { kind: "success".into(), total, files, elapsed_ms })
}
