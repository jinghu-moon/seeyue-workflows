// src/tools/compact_journal.rs
//
// Compact journal.jsonl: archive old entries, retain the most recent N lines.
// Inspired by Memory-Palace's compact_context tool.
//
// Behaviour:
//   - If total lines <= max_entries → returns already_compact, no writes.
//   - Otherwise: old lines → journal.archive-<ts>.jsonl (append mode),
//     recent max_entries lines overwrite journal.jsonl.
//   - summarize=true → appends event-type counts to session.yaml `notes`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CompactJournalParams {
    /// Maximum number of recent lines to retain (default: 200).
    pub max_entries: Option<usize>,
    /// Append event-type summary to session.yaml notes (default: false).
    #[serde(default)]
    pub summarize: bool,
}

#[derive(Debug, Serialize)]
pub struct CompactJournalResult {
    #[serde(rename = "type")]
    pub kind:           String, // "already_compact" | "compacted"
    pub total_before:   usize,
    pub archived:       usize,
    pub retained:       usize,
    pub archive_file:   Option<String>,
    pub summary_written: bool,
}

const DEFAULT_MAX: usize = 200;

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_compact_journal(
    params: CompactJournalParams,
    workflow_dir: &Path,
) -> Result<CompactJournalResult, ToolError> {
    let max_entries = params.max_entries.unwrap_or(DEFAULT_MAX).max(1);
    let journal_path = workflow_dir.join("journal.jsonl");

    // Read existing journal
    let content = if journal_path.exists() {
        fs::read_to_string(&journal_path)
            .map_err(|e| ToolError::IoError { message: format!("read journal: {e}") })?
    } else {
        return Ok(CompactJournalResult {
            kind:           "already_compact".into(),
            total_before:   0,
            archived:       0,
            retained:       0,
            archive_file:   None,
            summary_written: false,
        });
    };

    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    let total_before = lines.len();

    if total_before <= max_entries {
        return Ok(CompactJournalResult {
            kind:           "already_compact".into(),
            total_before,
            archived:       0,
            retained:       total_before,
            archive_file:   None,
            summary_written: false,
        });
    }

    let split_at   = total_before - max_entries;
    let old_lines  = &lines[..split_at];
    let keep_lines = &lines[split_at..];

    // Write archive file
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    let archive_name = format!("journal.archive-{ts}.jsonl");
    let archive_path = workflow_dir.join(&archive_name);

    {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&archive_path)
            .map_err(|e| ToolError::IoError { message: format!("open archive: {e}") })?;
        for line in old_lines {
            f.write_all(line.as_bytes())
                .and_then(|_| f.write_all(b"\n"))
                .map_err(|e| ToolError::IoError { message: format!("write archive: {e}") })?;
        }
        f.flush()
            .map_err(|e| ToolError::IoError { message: format!("flush archive: {e}") })?;
    }

    // Overwrite journal with retained lines
    {
        let mut new_content = keep_lines.join("\n");
        new_content.push('\n');
        fs::write(&journal_path, new_content)
            .map_err(|e| ToolError::IoError { message: format!("write journal: {e}") })?;
    }

    // Optional: append summary to session.yaml notes
    let summary_written = if params.summarize {
        write_summary_to_session(workflow_dir, old_lines, &archive_name)
    } else {
        false
    };

    Ok(CompactJournalResult {
        kind: "compacted".into(),
        total_before,
        archived: old_lines.len(),
        retained: keep_lines.len(),
        archive_file: Some(archive_name),
        summary_written,
    })
}

/// Append a compact summary (event-type counts) to session.yaml notes field.
/// Non-fatal: returns false on any error rather than propagating.
fn write_summary_to_session(workflow_dir: &Path, lines: &[&str], archive_name: &str) -> bool {
    let session_path = workflow_dir.join("session.yaml");
    if !session_path.exists() {
        return false;
    }

    // Count event types in archived lines
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for line in lines {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            let event = v.get("event")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown")
                .to_string();
            *counts.entry(event).or_insert(0) += 1;
        }
    }

    let summary_line = format!(
        "  - compact_journal: archived {} events into {} ({})",
        lines.len(),
        archive_name,
        counts.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Append to notes section or add it
    let Ok(mut yaml_str) = fs::read_to_string(&session_path) else { return false; };

    if yaml_str.contains("notes:") {
        // Append after the notes: key
        yaml_str = yaml_str.replacen(
            "notes:",
            &format!("notes:\n{summary_line}"),
            1,
        );
    } else {
        yaml_str.push_str(&format!("\nnotes:\n{summary_line}\n"));
    }

    fs::write(&session_path, yaml_str).is_ok()
}
