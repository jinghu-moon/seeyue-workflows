// src/tools/multi_file_edit.rs
//
// Cross-file atomic batch editing.
// Protocol: validate ALL edits across ALL files first → write atomically.
// Any validation failure → no files are modified.
// One checkpoint group per call (rewindable).

use std::path::Path;

use serde::Serialize;

use crate::storage::checkpoint::CheckpointStore;
use crate::storage::cache::ReadCache;
use crate::storage::backup::BackupManager;
use crate::error::ToolError;
use crate::tools::edit::apply_edit_in_memory;
use crate::treesitter::languages;

// ─── Constants ───────────────────────────────────────────────────────────────

const MAX_FILES: usize = 20;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct FileEditSet {
    pub file_path: String,
    pub edits:     Vec<FileEditItem>,
}

pub struct FileEditItem {
    pub old_string:  String,
    pub new_string:  String,
    pub replace_all: Option<bool>,
}

pub struct MultiFileEditParams {
    pub edits:         Vec<FileEditSet>,
    pub verify_syntax: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct MultiFileEditResult {
    pub status:         String,  // "ok" | "validation_failed"
    pub files_modified: usize,
    pub results:        Vec<FileEditOutcome>,
    pub checkpoint_id:  String,
}

#[derive(Debug, Serialize)]
pub struct FileEditOutcome {
    pub file_path:     String,
    pub edits_applied: usize,
    pub syntax_valid:  Option<bool>,
    pub lines_changed: i64,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_multi_file_edit(
    params: MultiFileEditParams,
    _cache:  &ReadCache,
    checkpoint: &CheckpointStore,
    _backup: &BackupManager,
    workspace: &Path,
) -> Result<MultiFileEditResult, ToolError> {
    if params.edits.is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "edits".to_string(),
            hint: "Provide at least one FileEditSet.".to_string(),
        });
    }
    if params.edits.len() > MAX_FILES {
        return Err(ToolError::MissingParameter {
            missing: "edits".to_string(),
            hint: format!("Maximum {} files per call, got {}.", MAX_FILES, params.edits.len()),
        });
    }

    let do_syntax = params.verify_syntax.unwrap_or(true);
    let call_id = format!("multi_file_edit_{}", chrono::Utc::now().timestamp_millis());

    // ── Phase 1: Validate all edits (no writes) ───────────────────────────
    let mut plans: Vec<(String, std::path::PathBuf, Vec<u8>, Vec<u8>)> = Vec::new();
    // (file_path_str, abs_path, original_bytes, new_bytes)

    for file_set in &params.edits {
        let abs = crate::platform::path::resolve(workspace, &file_set.file_path)
            .map_err(|e| ToolError::PathEscape {
                file_path: file_set.file_path.clone(),
                hint: format!("{:?}", e),
            })?;

        if !abs.exists() {
            return Err(ToolError::FileNotFound {
                file_path: file_set.file_path.clone(),
                hint: "File must exist before multi_file_edit.".to_string(),
            });
        }

        // Read original content
        let original_bytes = std::fs::read(&abs).map_err(|e| ToolError::IoError {
            message: format!("Cannot read {}: {}", file_set.file_path, e),
        })?;
        let mut current_text = String::from_utf8_lossy(&original_bytes).into_owned();

        // Apply edits sequentially in memory to validate
        for (idx, item) in file_set.edits.iter().enumerate() {
            // Special case: empty old_string = replace entire file content
            let new_text = if item.old_string.is_empty() {
                item.new_string.clone()
            } else {
                let result = apply_edit_in_memory(
                    &item.old_string,
                    &item.new_string,
                    item.replace_all.unwrap_or(false),
                    &current_text,
                ).map_err(|e| ToolError::EditFailed {
                    edit_index:   idx,
                    edit_preview: crate::error::EditPreview {
                        old_string: item.old_string.clone(),
                        new_string: item.new_string.clone(),
                    },
                    cause:      Box::new(e),
                    file_state: "unchanged (validation phase)".to_string(),
                    hint:       format!("Validation failed for {} edit #{}: No files were modified.", file_set.file_path, idx),
                })?;
                result.new_content
            };
            current_text = new_text;
        }

        let new_bytes = current_text.into_bytes();

        // Optional syntax check on new content
        if do_syntax {
            let lang = languages::detect_language(&abs);
            if !lang.is_empty() {
                if let Some(ts_lang) = languages::ts_language(&lang) {
                    let mut parser = tree_sitter::Parser::new();
                    let grammar = languages::grammar_for(ts_lang);
                    if parser.set_language(&grammar).is_ok() {
                        let text = String::from_utf8_lossy(&new_bytes);
                        if let Some(tree) = parser.parse(text.as_ref(), None) {
                            if tree.root_node().has_error() {
                                return Err(ToolError::SyntaxError {
                                    language: lang,
                                    errors:   Vec::new(),
                                    hint:     format!("Syntax error in {}. No files were modified.", file_set.file_path),
                                });
                            }
                        }
                    }
                }
            }
        }

        plans.push((file_set.file_path.clone(), abs, original_bytes, new_bytes));
    }

    // ── Phase 2: Checkpoint all files ────────────────────────────────────
    for (_, abs, _, _) in &plans {
        let _ = checkpoint.capture(abs, &call_id, "multi_file_edit");
    }

    // ── Phase 3: Atomic write ─────────────────────────────────────────────
    let mut outcomes: Vec<FileEditOutcome> = Vec::new();

    for (file_path_str, abs, original_bytes, new_bytes) in &plans {
        let lines_before = original_bytes.iter().filter(|&&b| b == b'\n').count() as i64;
        let lines_after  = new_bytes.iter().filter(|&&b| b == b'\n').count() as i64;

        std::fs::write(abs, new_bytes).map_err(|e| ToolError::IoError {
            message: format!("Write failed for {}: {}", file_path_str, e),
        })?;

        // Syntax validity of final written content
        let syntax_valid = if do_syntax {
            let lang = languages::detect_language(abs);
            if !lang.is_empty() {
                if let Some(ts_lang) = languages::ts_language(&lang) {
                    let mut parser = tree_sitter::Parser::new();
                    let grammar = languages::grammar_for(ts_lang);
                    if parser.set_language(&grammar).is_ok() {
                        let text = String::from_utf8_lossy(new_bytes);
                        parser.parse(text.as_ref(), None)
                            .map(|t| !t.root_node().has_error())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        outcomes.push(FileEditOutcome {
            file_path:     file_path_str.clone(),
            edits_applied: params.edits.iter()
                .find(|s| s.file_path == *file_path_str)
                .map(|s| s.edits.len())
                .unwrap_or(0),
            syntax_valid,
            lines_changed: lines_after - lines_before,
        });
    }

    Ok(MultiFileEditResult {
        status:         "ok".to_string(),
        files_modified: outcomes.len(),
        results:        outcomes,
        checkpoint_id:  call_id,
    })
}
