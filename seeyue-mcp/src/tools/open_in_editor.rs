// src/tools/open_in_editor.rs
//
// open_in_editor: Open a file in VS Code / Cursor at a specific line.
// Improves human-AI collaboration by letting the agent surface relevant code.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OpenInEditorParams {
    /// File path relative to workspace root.
    pub path:   String,
    /// 1-based line number to jump to (optional).
    pub line:   Option<usize>,
    /// 1-based column (optional).
    pub column: Option<usize>,
    /// Editor: "vscode" (default) | "cursor" | "auto" (tries cursor first).
    pub editor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpenInEditorResult {
    #[serde(rename = "type")]
    pub kind:    String, // "opened" | "not_found" | "no_editor"
    pub path:    String,
    pub line:    Option<usize>,
    pub editor:  String,
    pub command: String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_open_in_editor(
    params: OpenInEditorParams,
    workspace: &Path,
) -> Result<OpenInEditorResult, ToolError> {
    if params.path.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "path".into(),
            hint:    "Provide a file path relative to workspace root.".into(),
        });
    }

    let abs_path = workspace.join(&params.path);
    if !abs_path.exists() {
        return Ok(OpenInEditorResult {
            kind:    "not_found".into(),
            path:    params.path.clone(),
            line:    params.line,
            editor:  "none".into(),
            command: String::new(),
        });
    }

    let editor_pref = params.editor.as_deref().unwrap_or("auto");

    // Build --goto argument
    let goto_arg = match params.line {
        Some(line) => {
            let col = params.column.unwrap_or(1);
            format!("{}:{}:{}", abs_path.display(), line, col)
        }
        None => abs_path.display().to_string(),
    };

    let (cmd_name, editor_label) = resolve_editor(editor_pref)?;

    let status = std::process::Command::new(&cmd_name)
        .arg("--goto")
        .arg(&goto_arg)
        .spawn()
        .map_err(|e| ToolError::IoError {
            message: format!("Failed to launch {cmd_name}: {e}"),
        })?;
    drop(status); // fire-and-forget

    let display_cmd = format!("{} --goto {}", cmd_name, goto_arg);

    Ok(OpenInEditorResult {
        kind:    "opened".into(),
        path:    params.path,
        line:    params.line,
        editor:  editor_label,
        command: display_cmd,
    })
}

fn resolve_editor(pref: &str) -> Result<(String, String), ToolError> {
    match pref {
        "cursor" => Ok(("cursor".into(), "Cursor".into())),
        "vscode" => Ok(("code".into(), "VS Code".into())),
        _ => {
            // auto: try cursor first, then code
            if which_exists("cursor") {
                Ok(("cursor".into(), "Cursor".into()))
            } else if which_exists("code") {
                Ok(("code".into(), "VS Code".into()))
            } else {
                Err(ToolError::IoError {
                    message: "Neither 'cursor' nor 'code' found in PATH.".into(),
                })
            }
        }
    }
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("where")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
