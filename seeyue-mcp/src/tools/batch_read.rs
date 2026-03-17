// src/tools/batch_read.rs
//
// Read multiple files in a single request, reducing round-trips for agents.

use serde::{Deserialize, Serialize};
use std::path::Path;

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
    pub kind:    String, // "success"
    pub total:   usize,
    pub files:   Vec<FileReadEntry>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

const MAX_PATHS: usize = 20;

pub fn run_batch_read(
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

    let files: Vec<FileReadEntry> = params.paths.iter().map(|p| {
        match resolve_path(workspace, p) {
            Err(e) => FileReadEntry {
                path:    p.clone(),
                content: String::new(),
                size:    0,
                error:   Some(format!("{e:?}")),
            },
            Ok(abs) => match safe_read(&abs) {
                Err(e) => FileReadEntry {
                    path:    p.clone(),
                    content: String::new(),
                    size:    0,
                    error:   Some(format!("{e:?}")),
                },
                Ok(data) => {
                    let size = data.content.len();
                    FileReadEntry {
                        path:    p.clone(),
                        content: data.content,
                        size,
                        error:   None,
                    }
                }
            },
        }
    }).collect();

    let total = files.len();
    Ok(BatchReadResult { kind: "success".into(), total, files })
}
