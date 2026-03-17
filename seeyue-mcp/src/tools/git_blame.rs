// src/tools/git_blame.rs
//
// git blame — per-line authorship for a file.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::ToolError;
use crate::git;
use crate::tools::read::resolve_path;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GitBlameParams {
    pub path:       String,
    /// Line range: 1-based, inclusive. None = entire file.
    pub start_line: Option<usize>,
    pub end_line:   Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct BlameLine {
    pub line:    usize,
    pub hash:    String,
    pub short:   String,
    pub author:  String,
    pub date:    String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct GitBlameResult {
    #[serde(rename = "type")]
    pub kind:  String, // "success"
    pub path:  String,
    pub total: usize,
    pub lines: Vec<BlameLine>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_git_blame(
    params: GitBlameParams,
    workspace: &Path,
) -> Result<GitBlameResult, ToolError> {
    git::ensure_git_available()?;
    git::ensure_git_repo(workspace)?;

    let abs = resolve_path(workspace, &params.path)?;
    if !abs.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    // Build args: git blame --porcelain [-L start,end] -- <path>
    let mut args: Vec<String> = vec!["blame".into(), "--porcelain".into()];

    if let (Some(s), Some(e)) = (params.start_line, params.end_line) {
        args.push("-L".into());
        args.push(format!("{s},{e}"));
    } else if let Some(s) = params.start_line {
        args.push("-L".into());
        args.push(format!("{s},+9999"));
    }

    args.push("--".into());

    // Use relative path from workspace root for git
    let rel = abs.strip_prefix(workspace)
        .unwrap_or(&abs)
        .to_string_lossy()
        .replace('\\', "/");
    args.push(rel.clone());

    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = git::git_output(workspace, &args_ref)?;

    let lines = parse_porcelain(&output);
    let total = lines.len();

    Ok(GitBlameResult {
        kind: "success".into(),
        path: params.path,
        total,
        lines,
    })
}

// ─── Porcelain parser ─────────────────────────────────────────────────────────
//
// git blame --porcelain output format (per commit block):
//   <40-char-hash> <orig-line> <final-line> [<num-lines>]
//   author <name>
//   author-time <unix>
//   ... (other headers)
//   \t<content>

fn parse_porcelain(output: &str) -> Vec<BlameLine> {
    let mut lines = Vec::new();
    let mut hash   = String::new();
    let mut author = String::new();
    let mut date   = String::new();
    let mut lineno: usize = 0;

    for raw in output.lines() {
        if raw.starts_with('\t') {
            // content line
            let content = raw[1..].to_string();
            let short = if hash.len() >= 7 { hash[..7].to_string() } else { hash.clone() };
            lines.push(BlameLine {
                line:    lineno,
                hash:    hash.clone(),
                short,
                author:  author.clone(),
                date:    date.clone(),
                content,
            });
        } else if raw.len() > 40 && raw.chars().next().map_or(false, |c| c.is_ascii_hexdigit()) {
            // header line: <hash> <orig> <final> [count]
            let parts: Vec<&str> = raw.splitn(4, ' ').collect();
            hash   = parts[0].to_string();
            lineno = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        } else if let Some(rest) = raw.strip_prefix("author ") {
            author = rest.to_string();
        } else if let Some(rest) = raw.strip_prefix("author-time ") {
            // unix timestamp → ISO-ish string
            date = rest.to_string();
        }
    }

    lines
}
