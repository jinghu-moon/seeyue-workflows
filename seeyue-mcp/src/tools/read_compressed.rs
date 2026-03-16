use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::encoding::safe_read;
use crate::error::ToolError;
use crate::tools::read::resolve_path;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReadCompressedParams {
    pub path:         String,
    pub token_budget: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ReadCompressedResult {
    #[serde(rename = "type")]
    pub kind:              String, // "success"
    pub path:              String,
    pub token_budget:      usize,
    pub token_estimate:    usize,
    pub compression_level: u8,
    pub content:           String,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_read_compressed(
    params: ReadCompressedParams,
    workspace: &Path,
) -> Result<ReadCompressedResult, ToolError> {
    let path = resolve_path(workspace, &params.path)?;
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let original = file_data.content;

    let budget = params.token_budget.unwrap_or(800).max(50);
    let mut level: u8 = 0;
    let mut content = original.clone();
    let mut estimate = estimate_tokens(&content);

    if estimate > budget {
        level = 1;
        content = collapse_blank_lines(&content);
        estimate = estimate_tokens(&content);
    }
    if estimate > budget {
        level = 2;
        content = strip_comments(&content);
        estimate = estimate_tokens(&content);
    }
    if estimate > budget {
        level = 3;
        content = strip_imports(&content);
        estimate = estimate_tokens(&content);
    }
    if estimate > budget {
        level = 4;
        content = skeletonize(&content);
        estimate = estimate_tokens(&content);
    }

    Ok(ReadCompressedResult {
        kind: "success".into(),
        path: params.path,
        token_budget: budget,
        token_estimate: estimate,
        compression_level: level,
        content,
    })
}

// ─── 压缩规则 ────────────────────────────────────────────────────────────────

fn estimate_tokens(content: &str) -> usize {
    content.len() / 4
}

fn collapse_blank_lines(content: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut blank_count = 0;
    for line in content.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                out.push(String::new());
            }
        } else {
            blank_count = 0;
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn strip_comments(content: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if in_block {
            if trimmed.contains("*/") {
                in_block = false;
            }
            continue;
        }

        if trimmed.starts_with("/*") {
            in_block = true;
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with("#") {
            continue;
        }
        if trimmed.starts_with("#[") {
            continue;
        }
        if trimmed.starts_with("*") && trimmed.ends_with("*/") {
            continue;
        }

        out.push(line.to_string());
    }

    out.join("\n")
}

fn strip_imports(content: &str) -> String {
    let mut out = Vec::new();
    let mut removed = 0usize;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let is_import = trimmed.starts_with("use ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("from ")
            || trimmed.starts_with("#include")
            || trimmed.starts_with("require(")
            || trimmed.starts_with("mod ");

        if is_import {
            removed += 1;
            continue;
        }

        if removed > 0 {
            out.push(format!("// ... {removed} imports omitted"));
            removed = 0;
        }
        out.push(line.to_string());
    }

    if removed > 0 {
        out.push(format!("// ... {removed} imports omitted"));
    }

    out.join("\n")
}

fn skeletonize(content: &str) -> String {
    let mut out = Vec::new();
    let mut keep_next = 0usize;
    let mut omitted = 0usize;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let is_signature = is_fn_signature(trimmed);

        if is_signature {
            if omitted > 0 {
                out.push(format!("// ... {omitted} lines omitted"));
                omitted = 0;
            }
            out.push(line.to_string());
            keep_next = 3;
            continue;
        }

        if keep_next > 0 {
            out.push(line.to_string());
            keep_next -= 1;
            continue;
        }

        omitted += 1;
    }

    if omitted > 0 {
        out.push(format!("// ... {omitted} lines omitted"));
    }

    out.join("\n")
}

fn is_fn_signature(line: &str) -> bool {
    let trimmed = line.trim();
    let keywords = [
        "fn ", "pub fn ", "async fn ", "pub async fn ",
        "def ", "function ", "async function ", "func ",
        "class ", "struct ", "interface ", "trait ",
    ];
    keywords.iter().any(|k| trimmed.starts_with(k))
}
