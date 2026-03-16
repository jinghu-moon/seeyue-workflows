use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::ToolError;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ResolvePathParams {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ResolvePathResult {
    #[serde(rename = "type")]
    pub kind:         String, // "success"
    pub input:        String,
    pub absolute:     String,
    pub relative:     String,
    pub exists:       bool,
    pub is_dir:       bool,
    pub in_workspace: bool,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_resolve_path(
    params: ResolvePathParams,
    workspace: &Path,
) -> Result<ResolvePathResult, ToolError> {
    let resolved = crate::platform::path::resolve(workspace, &params.path)?;
    let exists   = resolved.exists();
    let is_dir   = resolved.is_dir();

    let absolute = resolved.to_string_lossy().to_string();
    let relative = resolve_relative(&resolved, workspace).unwrap_or_else(|| ".".into());

    Ok(ResolvePathResult {
        kind:         "success".into(),
        input:        params.path,
        absolute,
        relative,
        exists,
        is_dir,
        in_workspace: true,
    })
}

// ─── 辅助 ────────────────────────────────────────────────────────────────────

fn resolve_relative(resolved: &Path, workspace: &Path) -> Option<String> {
    if let Ok(rel) = resolved.strip_prefix(workspace) {
        return Some(normalize_separators(&rel.to_string_lossy()));
    }

    // fallback: UNC 前缀差异时尝试字符串前缀匹配
    let abs_buf = resolved.to_string_lossy().to_string();
    let ws_buf  = workspace.to_string_lossy().to_string();
    let abs = strip_unc_prefix(&abs_buf);
    let ws  = strip_unc_prefix(&ws_buf);

    let abs_lower = abs.to_lowercase();
    let ws_lower  = ws.to_lowercase();

    if abs_lower.starts_with(&ws_lower) {
        let mut rel = abs[ws.len()..].to_string();
        rel = rel.trim_start_matches(['\\', '/']).to_string();
        if rel.is_empty() {
            return Some(".".into());
        }
        return Some(normalize_separators(&rel));
    }
    None
}

fn strip_unc_prefix(s: &str) -> &str {
    s.strip_prefix(r"\\?\\").unwrap_or(s)
}

fn normalize_separators(s: &str) -> String {
    if cfg!(windows) {
        s.replace('/', "\\")
    } else {
        s.replace('\\', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_relative_basic() {
        let ws = PathBuf::from(r"C:\\workspace");
        let p  = PathBuf::from(r"C:\\workspace\\src\\main.rs");
        let rel = resolve_relative(&p, &ws).unwrap();
        assert_eq!(rel, r"src\\main.rs");
    }
}
