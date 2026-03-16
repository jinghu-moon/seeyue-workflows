use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::treesitter::{languages, symbols};
use crate::treesitter::symbols::Symbol;
use crate::tools::read::resolve_path;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FileOutlineParams {
    pub path:  String,
    pub depth: Option<u8>, // 0=仅顶层, 1=含方法(默认), 2=全展开
}

#[derive(Debug, Serialize)]
pub struct FileOutlineResult {
    #[serde(rename = "type")]
    pub kind:           String, // "success"
    pub path:           String,
    pub language:       String,
    pub total_lines:    usize,
    pub token_estimate: usize,
    pub symbols:        Vec<Symbol>,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_file_outline(
    params: FileOutlineParams,
    workspace: &Path,
) -> Result<FileOutlineResult, ToolError> {
    let path = resolve_path(workspace, &params.path)?;

    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let content = file_data.content;
    let total_lines = content.lines().count();

    let language = languages::detect_language(&path);
    let depth = params.depth.unwrap_or(1).min(2);

    let symbols = symbols::extract_symbols(&language, &content, depth);
    let token_estimate = symbols::estimate_tokens(&symbols);

    Ok(FileOutlineResult {
        kind: "success".into(),
        path: params.path,
        language,
        total_lines,
        token_estimate,
        symbols,
    })
}
