use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

use crate::encoding::safe_read;
use crate::error::{SyntaxIssue, ToolError};
use crate::treesitter::languages;
use crate::tools::read::resolve_path;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct VerifySyntaxParams {
    pub path:     Option<String>,
    pub content:  Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerifySyntaxResult {
    #[serde(rename = "type")]
    pub kind:     String, // "success"
    pub valid:    bool,
    pub language: String,
    pub parse_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path:     Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note:     Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors:   Option<Vec<SyntaxIssue>>,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_verify_syntax(
    params: VerifySyntaxParams,
    workspace: &Path,
) -> Result<VerifySyntaxResult, ToolError> {
    let (source, language, path_out) = if let Some(path) = params.path {
        let resolved = resolve_path(workspace, &path)?;
        if !resolved.exists() {
            return Err(ToolError::FileNotFound {
                file_path: path.clone(),
                hint: "File does not exist.".into(),
            });
        }
        let file_data = safe_read(&resolved)?;
        let language = params.language.unwrap_or_else(|| languages::detect_language(&resolved));
        (file_data.content, language, Some(path))
    } else if let Some(src) = params.content {
        let language = params.language.ok_or_else(|| ToolError::MissingParameter {
            missing: "language".into(),
            hint: "When content is provided, language is required (e.g. rust, python, typescript).".into(),
        })?;
        (src, language, None)
    } else {
        return Err(ToolError::MissingParameter {
            missing: "path|content".into(),
            hint: "Provide either path or content for syntax verification.".into(),
        });
    };

    let ts_lang = languages::ts_language(&language);
    if ts_lang.is_none() {
        return Ok(VerifySyntaxResult {
            kind: "success".into(),
            valid: true,
            language,
            parse_ms: 0,
            path: path_out,
            note: Some("No tree-sitter grammar for this language — skipped (treated as valid).".into()),
            errors: None,
        });
    }

    let mut parser = tree_sitter::Parser::new();
    let grammar = languages::grammar_for(ts_lang.unwrap());
    if parser.set_language(&grammar).is_err() {
        return Err(ToolError::UnsupportedLanguage {
            language,
            hint: "Tree-sitter grammar initialization failed.".into(),
        });
    }

    let start = Instant::now();
    let tree = parser
        .parse(&source, None)
        .ok_or_else(|| ToolError::SyntaxError {
            language: language.clone(),
            errors: Vec::new(),
            hint: "tree-sitter parse failed (timeout or internal error).".into(),
        })?;
    let parse_ms = start.elapsed().as_millis();

    let mut errors: Vec<SyntaxIssue> = Vec::new();
    collect_errors(tree.root_node(), source.as_bytes(), &mut errors);

    let valid = errors.is_empty();

    Ok(VerifySyntaxResult {
        kind: "success".into(),
        valid,
        language,
        parse_ms,
        path: path_out,
        note: None,
        errors: if valid { None } else { Some(errors) },
    })
}

// ─── 错误收集 ────────────────────────────────────────────────────────────────

fn collect_errors(node: tree_sitter::Node, src: &[u8], out: &mut Vec<SyntaxIssue>) {
    if node.is_error() || node.is_missing() {
        let row = node.start_position().row;
        let col = node.start_position().column;

        let message = if node.is_missing() {
            format!("missing `{}`", node.kind())
        } else {
            let found = node.utf8_text(src).unwrap_or("?");
            let trimmed = found.trim().chars().take(30).collect::<String>();
            if trimmed.is_empty() {
                format!("unexpected end of input near col {}", col + 1)
            } else {
                format!("unexpected token `{}`", trimmed)
            }
        };

        let text = std::str::from_utf8(src).unwrap_or("");
        let line_text = text.lines().nth(row).unwrap_or("");
        let display = line_text.chars().take(80).collect::<String>();
        let caret_col = col.min(display.len());
        let snippet = format!("{}\n{}^", display, " ".repeat(caret_col));

        out.push(SyntaxIssue {
            line: row + 1,
            column: col + 1,
            kind: if node.is_missing() { "MISSING".into() } else { "ERROR".into() },
            message,
            snippet: Some(snippet),
        });
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_errors(child, src, out);
        }
    }
}
