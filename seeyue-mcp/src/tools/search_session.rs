// src/tools/search_session.rs
//
// Search journal.jsonl for matching entries.
// Provides structured filtering by event type, phase, node, and free-text query.
// More focused than search_workspace — targets session state artifacts only.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchSessionParams {
    /// Free-text query matched against the full JSON line.
    pub query: String,
    /// Filter by exact event type (e.g. "write_recorded").
    pub filter_event: Option<String>,
    /// Filter by phase id/name.
    pub filter_phase: Option<String>,
    /// Filter by node id/name.
    pub filter_node: Option<String>,
    /// Maximum results to return (default: 20, max: 200).
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct JournalEntry {
    pub ts:              String,
    pub event:           String,
    pub phase:           Option<String>,
    pub node_id:         Option<String>,
    pub actor:           Option<String>,
    pub payload_preview: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchSessionResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success"
    pub query:     String,
    pub total:     usize,
    pub truncated: bool,
    pub entries:   Vec<JournalEntry>,
}

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT:     usize = 200;
const PREVIEW_LEN:   usize = 120;

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_search_session(
    params: SearchSessionParams,
    workflow_dir: &Path,
) -> Result<SearchSessionResult, ToolError> {
    if params.query.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "query".into(),
            hint: "Provide a non-empty search query.".into(),
        });
    }

    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let journal_path = workflow_dir.join("journal.jsonl");

    let content = if journal_path.exists() {
        fs::read_to_string(&journal_path)
            .map_err(|e| ToolError::IoError { message: format!("read journal: {e}") })?
    } else {
        String::new()
    };

    let query_lower = params.query.to_lowercase();

    let mut matched: Vec<JournalEntry> = Vec::new();
    let mut total = 0usize;

    // Iterate in reverse so most recent entries come first
    for line in content.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }

        // Free-text match on raw JSON line
        if !line.to_lowercase().contains(&query_lower) {
            continue;
        }

        // Parse for structured filters
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("").to_string();
        let phase = v.get("phase").and_then(|e| e.as_str()).map(|s| s.to_string());
        let node_id = v.get("node_id").and_then(|e| e.as_str()).map(|s| s.to_string());

        // Structured filters
        if let Some(ref fe) = params.filter_event {
            if !event.eq_ignore_ascii_case(fe) {
                continue;
            }
        }
        if let Some(ref fp) = params.filter_phase {
            match &phase {
                Some(p) if p.eq_ignore_ascii_case(fp) => {}
                _ => continue,
            }
        }
        if let Some(ref fn_) = params.filter_node {
            match &node_id {
                Some(n) if n.eq_ignore_ascii_case(fn_) => {}
                _ => continue,
            }
        }

        total += 1;

        if matched.len() < limit {
            let ts    = v.get("ts").and_then(|e| e.as_str()).unwrap_or("").to_string();
            let actor = v.get("actor").and_then(|e| e.as_str()).map(|s| s.to_string());
            let payload_preview = v.get("payload").map(|p| {
                let s = serde_json::to_string(p).unwrap_or_default();
                if s.len() > PREVIEW_LEN {
                    format!("{}…", &s[..PREVIEW_LEN])
                } else {
                    s
                }
            });

            matched.push(JournalEntry {
                ts,
                event,
                phase,
                node_id,
                actor,
                payload_preview,
            });
        }
    }

    Ok(SearchSessionResult {
        kind:      "success".into(),
        query:     params.query,
        total,
        truncated: total > limit,
        entries:   matched,
    })
}
