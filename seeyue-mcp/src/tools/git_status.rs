use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

use crate::error::ToolError;
use crate::git;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GitStatusResult {
    #[serde(rename = "type")]
    pub kind:       String, // "success"
    pub branch:     String,
    pub modified:   Vec<String>,
    pub added:      Vec<String>,
    pub deleted:    Vec<String>,
    pub untracked:  Vec<String>,
    pub staged:     Vec<String>,
    pub conflicts:  Vec<String>,
    pub clean:      bool,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_git_status(workspace: &Path) -> Result<GitStatusResult, ToolError> {
    git::ensure_git_available()?;
    git::ensure_git_repo(workspace)?;

    let branch = git::git_output(workspace, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();

    let status_raw = git::git_output(workspace, &["status", "--porcelain=v1", "-u"])?;

    let mut modified:  BTreeSet<String> = BTreeSet::new();
    let mut added:     BTreeSet<String> = BTreeSet::new();
    let mut deleted:   BTreeSet<String> = BTreeSet::new();
    let mut untracked: BTreeSet<String> = BTreeSet::new();
    let mut staged:    BTreeSet<String> = BTreeSet::new();
    let mut conflicts: BTreeSet<String> = BTreeSet::new();

    for line in status_raw.lines() {
        if line.len() < 2 {
            continue;
        }
        if line.starts_with("??") {
            let path = normalize_path(line[2..].trim());
            untracked.insert(path);
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        let xy = &line[..2];
        let raw_path = line[3..].trim();
        let path = normalize_path(parse_rename_path(raw_path));

        let x = xy.chars().next().unwrap_or(' ');
        let y = xy.chars().nth(1).unwrap_or(' ');

        if is_conflict(x, y) {
            conflicts.insert(path);
            continue;
        }

        if x == 'A' {
            added.insert(path.clone());
        }
        if x == 'M' {
            staged.insert(path.clone());
        }
        if x == 'D' {
            deleted.insert(path.clone());
        }
        if y == 'M' {
            modified.insert(path.clone());
        }
        if y == 'D' {
            deleted.insert(path.clone());
        }
        if x == 'R' || y == 'R' || x == 'C' || y == 'C' {
            modified.insert(path);
        }
    }

    let clean = modified.is_empty()
        && added.is_empty()
        && deleted.is_empty()
        && untracked.is_empty()
        && staged.is_empty()
        && conflicts.is_empty();

    Ok(GitStatusResult {
        kind:      "success".into(),
        branch,
        modified:  modified.into_iter().collect(),
        added:     added.into_iter().collect(),
        deleted:   deleted.into_iter().collect(),
        untracked: untracked.into_iter().collect(),
        staged:    staged.into_iter().collect(),
        conflicts: conflicts.into_iter().collect(),
        clean,
    })
}

// ─── 辅助 ────────────────────────────────────────────────────────────────────

fn is_conflict(x: char, y: char) -> bool {
    matches!(
        (x, y),
        ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D')
    )
}

fn parse_rename_path(raw: &str) -> &str {
    if let Some(idx) = raw.rfind("->") {
        return raw[idx + 2..].trim();
    }
    raw
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}
