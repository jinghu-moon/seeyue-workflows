use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::render::diff::DiffResult;
use crate::encoding::{detect_encoding, safe_read};
use crate::error::ToolError;
use crate::git;
use crate::tools::read::resolve_path;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GitDiffFileParams {
    pub path:   String,
    pub base:   Option<String>,
    pub staged: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GitDiffFileResult {
    #[serde(rename = "type")]
    pub kind:   String, // "success"
    pub path:   String,
    pub base:   String,
    pub staged: bool,
    pub diff:   DiffResult,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_git_diff_file(
    params: GitDiffFileParams,
    workspace: &Path,
) -> Result<GitDiffFileResult, ToolError> {
    git::ensure_git_available()?;
    git::ensure_git_repo(workspace)?;

    let base = params.base.unwrap_or_else(|| "HEAD".into());
    let staged = params.staged.unwrap_or(false);

    let absolute = resolve_path(workspace, &params.path)?;
    let rel_path = to_repo_rel(&absolute, workspace)?;
    let display_path = rel_path.replace('\\', "/");

    let base_spec = format!("{base}:{rel_path}");
    let base_bytes = read_git_blob(workspace, &base_spec)?;
    let base_content = decode_bytes(base_bytes.as_deref().unwrap_or(&[]));

    let new_content = if staged {
        let index_spec = format!(":{}", rel_path);
        let index_bytes = read_git_blob(workspace, &index_spec)?;
        decode_bytes(index_bytes.as_deref().unwrap_or(&[]))
    } else if absolute.exists() {
        safe_read(&absolute)?.content
    } else {
        String::new()
    };

    let diff = crate::render::diff::compute_diff(&display_path, &base_content, &new_content, None);

    Ok(GitDiffFileResult {
        kind: "success".into(),
        path: display_path,
        base,
        staged,
        diff,
    })
}

// ─── 辅助 ────────────────────────────────────────────────────────────────────

fn to_repo_rel(path: &Path, workspace: &Path) -> Result<String, ToolError> {
    let rel = path.strip_prefix(workspace).map_err(|_| ToolError::PathEscape {
        file_path: path.to_string_lossy().to_string(),
        hint: "Path is outside workspace.".into(),
    })?;
    Ok(rel.to_string_lossy().to_string())
}

fn read_git_blob(repo: &Path, spec: &str) -> Result<Option<Vec<u8>>, ToolError> {
    let out = git::git_output_bytes_allow(repo, &["show", spec], &[0, 128])?;
    if out.status.success() {
        return Ok(Some(out.stdout));
    }
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    if is_missing_path(&stderr) {
        return Ok(None);
    }
    Err(ToolError::GitError {
        message: format!("git show {}: {}", spec, stderr.trim()),
        hint: "Verify git base reference and file path.".into(),
    })
}

fn is_missing_path(stderr: &str) -> bool {
    stderr.contains("exists on disk, but not in")
        || stderr.contains("path") && stderr.contains("does not exist")
        || stderr.contains("unknown revision or path not in the working tree")
}

fn decode_bytes(raw: &[u8]) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let enc_info = detect_encoding(raw);
    let enc = encoding_rs::Encoding::for_label(enc_info.name.as_bytes())
        .unwrap_or(encoding_rs::UTF_8);
    let (cow, _, _) = enc.decode(raw);
    cow.into_owned()
}
