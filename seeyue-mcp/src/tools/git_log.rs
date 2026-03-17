// src/tools/git_log.rs
//
// git log — structured commit history for a workspace or file path.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::ToolError;
use crate::git;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GitLogParams {
    /// Max number of commits to return (default: 20, max: 200)
    pub limit:  Option<usize>,
    /// Restrict to commits touching this relative path (optional)
    pub path:   Option<String>,
    /// Starting ref / branch / tag (default: HEAD)
    pub since:  Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommitEntry {
    pub hash:    String,
    pub short:   String,
    pub author:  String,
    pub date:    String,
    pub subject: String,
}

#[derive(Debug, Serialize)]
pub struct GitLogResult {
    #[serde(rename = "type")]
    pub kind:    String, // "success"
    pub total:   usize,
    pub commits: Vec<CommitEntry>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_git_log(
    params: GitLogParams,
    workspace: &Path,
) -> Result<GitLogResult, ToolError> {
    git::ensure_git_available()?;
    git::ensure_git_repo(workspace)?;

    let limit = params.limit.unwrap_or(20).min(200);
    let since = params.since.unwrap_or_else(|| "HEAD".into());

    // Use --format=VALUE (single arg) to avoid git misinterpreting the value as a revision.
    // Fields separated by ||| — unlikely in commit messages.
    let fmt_arg = "--format=%H|||%h|||%an|||%ai|||%s".to_string();
    let limit_str = format!("-{limit}");

    // Build arg list without path first
    let mut args: Vec<String> = vec![
        "log".into(),
        fmt_arg,
        limit_str,
        since,
        "--".into(),
    ];

    // Optional path filter — validated to stay inside workspace
    if let Some(ref p) = params.path {
        let abs = workspace.join(p);
        let canon_ws = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
        let canon_p  = abs.canonicalize().unwrap_or(abs.clone());
        if !canon_p.starts_with(&canon_ws) {
            return Err(ToolError::PathEscape {
                file_path: p.clone(),
                hint: "Path must be inside the workspace.".into(),
            });
        }
        args.push(p.clone());
    }

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = git::git_output(workspace, &args_ref)?;

    let commits: Vec<CommitEntry> = output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, "|||").collect();
            if parts.len() == 5 {
                Some(CommitEntry {
                    hash:    parts[0].to_string(),
                    short:   parts[1].to_string(),
                    author:  parts[2].to_string(),
                    date:    parts[3].to_string(),
                    subject: parts[4].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    let total = commits.len();
    Ok(GitLogResult { kind: "success".into(), total, commits })
}
