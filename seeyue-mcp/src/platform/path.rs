// src/platform/path.rs
//
// Windows 路径三大陷阱：
//   1. 正/反斜杠混用（Agent 发来任意格式）
//   2. MAX_PATH = 260（超出需要 \\?\ 前缀）
//   3. 路径逃逸（.. 穿越 workspace）

use std::path::{Component, Path, PathBuf};
use crate::error::ToolError;

const MAX_PATH: usize = 260;

/// 将 Agent 提供的路径安全地解析为 workspace 内的绝对路径
pub fn resolve(workspace: &Path, input: &str) -> Result<PathBuf, ToolError> {
    // Step 1: 统一为反斜杠（Windows native），再交给 PathBuf 处理
    let normalized = if cfg!(windows) {
        input.replace('/', "\\")
    } else {
        input.replace('\\', "/")
    };

    // Step 2: 拼接 workspace，手动折叠 . 和 ..（不依赖 fs::canonicalize，文件可能不存在）
    let joined    = workspace.join(&normalized);
    let collapsed = collapse_dotdot(&joined);

    // Step 3: 路径逃逸检查（防止 ../../etc/passwd 之类）
    // 使用 Path::starts_with() 进行组件级比较，天然处理正/反斜杠混用
    // workspace 同样经过 collapse_dotdot 以确保规范化
    let ws_collapsed = collapse_dotdot(workspace);
    if !collapsed.starts_with(&ws_collapsed) {
        return Err(ToolError::PathEscape {
            file_path: input.to_string(),
            hint: "Path resolves outside workspace root.".into(),
        });
    }

    // Step 4: Windows 长路径支持（> 260 字符）
    #[cfg(windows)]
    let final_path = if collapsed.to_string_lossy().len() > MAX_PATH {
        extended_prefix(collapsed)
    } else {
        collapsed
    };

    #[cfg(not(windows))]
    let final_path = collapsed;

    Ok(final_path)
}

/// 手动折叠 `.` 和 `..`，不调用 fs::canonicalize
fn collapse_dotdot(path: &Path) -> PathBuf {
    let mut stack: Vec<Component> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => { stack.pop(); }
            Component::CurDir    => {}
            other                => stack.push(other),
        }
    }
    stack.iter().collect()
}

/// 添加 \\?\ 前缀启用 Windows 长路径（> MAX_PATH = 260）
#[cfg(windows)]
fn extended_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        path
    } else {
        PathBuf::from(format!(r"\\?\{s}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_forward_slash() {
        let ws = PathBuf::from(r"C:\workspace");
        let p  = resolve(&ws, "src/main.rs").unwrap();
        assert!(p.to_string_lossy().contains("main.rs"));
    }

    #[test]
    fn reject_path_escape() {
        let ws = PathBuf::from(r"C:\workspace");
        let r  = resolve(&ws, "../../etc/passwd");
        assert!(r.is_err(), "should reject path escape");
        if let Err(ToolError::PathEscape { .. }) = r {} else { panic!("wrong error type"); }
    }

    #[test]
    fn allow_nested_path() {
        let ws = PathBuf::from(r"C:\workspace");
        let p  = resolve(&ws, r"src\auth\jwt.rs").unwrap();
        assert!(p.to_string_lossy().contains("jwt.rs"));
    }

    #[test]
    fn resolve_forward_slash_workspace() {
        // workspace 以正斜杠传入（env var 常见格式），路径仍应解析成功
        let ws = PathBuf::from("C:/workspace");
        let p  = resolve(&ws, "src/main.rs").unwrap();
        assert!(p.to_string_lossy().contains("main.rs"));
    }

    #[test]
    fn reject_escape_with_forward_slash_workspace() {
        let ws = PathBuf::from("C:/workspace");
        let r  = resolve(&ws, "../../etc/passwd");
        assert!(r.is_err());
    }
}
