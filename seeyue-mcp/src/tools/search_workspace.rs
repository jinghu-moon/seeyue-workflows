use serde::{Deserialize, Serialize};
use std::path::Path;

use ignore::WalkBuilder;
use regex::Regex;

use crate::error::ToolError;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchWorkspaceParams {
    pub pattern:       String,
    pub is_regex:      Option<bool>,
    pub file_glob:     Option<String>,
    pub context_lines: Option<usize>,
    pub max_results:   Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub path:           String,
    pub line:           usize,
    pub column:         usize,
    pub content:        String,
    pub context_before: Vec<String>,
    pub context_after:  Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchWorkspaceResult {
    #[serde(rename = "type")]
    pub kind:          String, // "success"
    pub pattern:       String,
    pub total_matches: usize,
    pub truncated:     bool,
    pub matches:       Vec<SearchMatch>,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_search_workspace(
    params: SearchWorkspaceParams,
    workspace: &Path,
) -> Result<SearchWorkspaceResult, ToolError> {
    if params.pattern.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "pattern".into(),
            hint: "pattern must be non-empty.".into(),
        });
    }

    let is_regex = params.is_regex.unwrap_or(false);
    let context = params.context_lines.unwrap_or(0);
    let max_results = params.max_results.unwrap_or(50);

    let regex = if is_regex {
        Some(Regex::new(&params.pattern).map_err(|e| ToolError::InvalidRegex {
            pattern: params.pattern.clone(),
            message: e.to_string(),
            hint: "Fix the regex pattern and retry.".into(),
        })?)
    } else {
        None
    };

    let globset = if let Some(glob) = params.file_glob.as_deref() {
        let glob = globset::Glob::new(glob).map_err(|e| ToolError::InvalidRegex {
            pattern: glob.to_string(),
            message: e.to_string(),
            hint: "Invalid file_glob pattern.".into(),
        })?;
        Some(globset::GlobSetBuilder::new().add(glob).build().unwrap())
    } else {
        None
    };

    let mut builder = WalkBuilder::new(workspace);
    builder.standard_filters(true);
    builder.follow_links(false);

    let mut matches: Vec<SearchMatch> = Vec::new();
    let mut total_matches = 0usize;

    'outer: for entry in builder.build().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(gs) = &globset {
            if !gs.is_match(path) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let (matched, col) = if let Some(re) = &regex {
                re.find(line).map(|m| (true, m.start() + 1)).unwrap_or((false, 0))
            } else {
                line.find(&params.pattern).map(|p| (true, p + 1)).unwrap_or((false, 0))
            };
            if !matched {
                continue;
            }

            total_matches += 1;
            if matches.len() >= max_results {
                break 'outer;
            }

            let line_no = idx + 1;
            let before_start = idx.saturating_sub(context);
            let after_end = (idx + 1 + context).min(lines.len());

            let context_before = lines[before_start..idx]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{}\t{}", before_start + i + 1, l))
                .collect();
            let context_after = lines[idx + 1..after_end]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{}\t{}", idx + 2 + i, l))
                .collect();

            let rel = path.strip_prefix(workspace)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            matches.push(SearchMatch {
                path: rel,
                line: line_no,
                column: col,
                content: line.trim_end().to_string(),
                context_before,
                context_after,
            });
        }
    }

    Ok(SearchWorkspaceResult {
        kind: "success".into(),
        pattern: params.pattern,
        total_matches,
        truncated: total_matches > matches.len(),
        matches,
    })
}
