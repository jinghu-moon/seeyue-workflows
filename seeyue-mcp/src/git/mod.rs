use std::path::Path;
use std::process::{Command, Output};

use crate::error::ToolError;

// ─── Git 预检 ────────────────────────────────────────────────────────────────

pub fn ensure_git_available() -> Result<(), ToolError> {
    if which::which("git").is_err() {
        return Err(ToolError::GitNotAvailable {
            hint: "git is not available in PATH. Install Git or update PATH.".into(),
        });
    }
    Ok(())
}

pub fn ensure_git_repo(repo: &Path) -> Result<(), ToolError> {
    let out = run_git(repo, &["rev-parse", "--is-inside-work-tree"])?;
    if !out.status.success() {
        return Err(ToolError::GitNotRepo {
            hint: "Workspace is not a git repository.".into(),
        });
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim() != "true" {
        return Err(ToolError::GitNotRepo {
            hint: "Workspace is not a git repository.".into(),
        });
    }
    Ok(())
}

// ─── Git 命令封装 ────────────────────────────────────────────────────────────

pub fn git_output(repo: &Path, args: &[&str]) -> Result<String, ToolError> {
    let out = run_git(repo, args)?;
    if !out.status.success() {
        return Err(ToolError::GitError {
            message: format!("git {}: {}", args.join(" "), stderr_text(&out)),
            hint: "Verify git repository state and retry.".into(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn git_output_bytes_allow(
    repo: &Path,
    args: &[&str],
    allowed: &[i32],
) -> Result<Output, ToolError> {
    let out = run_git(repo, args)?;
    let code = out.status.code().unwrap_or(-1);
    if !out.status.success() && !allowed.contains(&code) {
        return Err(ToolError::GitError {
            message: format!("git {}: {}", args.join(" "), stderr_text(&out)),
            hint: "Verify git repository state and retry.".into(),
        });
    }
    Ok(out)
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Output, ToolError> {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| ToolError::GitError {
            message: format!("git spawn failed: {}", e),
            hint: "Ensure git is installed and accessible.".into(),
        })
}

fn stderr_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}
