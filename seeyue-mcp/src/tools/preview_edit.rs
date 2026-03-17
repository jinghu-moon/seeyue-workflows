use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::render::diff::DiffResult;
use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::edit::apply_edit_in_memory;
use crate::tools::read::resolve_path;
use crate::tools::verify_syntax;
use crate::treesitter::languages;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PreviewEditParams {
    pub file_path:  String,
    pub old_string: String,
    pub new_string: String,
    pub replace_all: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PreviewEditResult {
    #[serde(rename = "type")]
    pub kind:              String, // "success"
    pub would_apply:       bool,
    pub replacements:      usize,
    pub match_kind:        String,
    pub syntax_valid_after: bool,
    pub diff:              DiffResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax:            Option<verify_syntax::VerifySyntaxResult>,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_preview_edit(
    params: PreviewEditParams,
    workspace: &Path,
) -> Result<PreviewEditResult, ToolError> {
    let path = resolve_path(workspace, &params.file_path)?;

    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.file_path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let original = file_data.content;

    let applied = apply_edit_in_memory(
        &params.old_string,
        &params.new_string,
        params.replace_all.unwrap_or(false),
        &original,
    )?;

    let diff = crate::render::diff::compute_diff(&params.file_path, &original, &applied.new_content, None);

    let language = languages::detect_language(&path);
    let syntax = verify_syntax::run_verify_syntax(
        verify_syntax::VerifySyntaxParams {
            path: None,
            content: Some(applied.new_content.clone()),
            language: Some(language),
        },
        workspace,
    )?;

    let syntax_valid = syntax.valid;

    Ok(PreviewEditResult {
        kind: "success".into(),
        would_apply: applied.replacements > 0,
        replacements: applied.replacements,
        match_kind: applied.match_kind,
        syntax_valid_after: syntax_valid,
        diff,
        syntax: if syntax_valid { None } else { Some(syntax) },
    })
}
