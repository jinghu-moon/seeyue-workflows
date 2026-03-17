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
    /// Sort order: "time" (default, newest first) | "event_weight" (high-value events first).
    pub sort_by: Option<String>,
    /// Include only events at or after this ISO 8601 timestamp (e.g. "2026-03-17T00:00:00Z").
    pub since: Option<String>,
    /// Include only events at or before this ISO 8601 timestamp.
    pub until: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JournalEntry {
    pub ts:              String,
    pub event:           String,
    pub phase:           Option<String>,
    pub node_id:         Option<String>,
    pub actor:           Option<String>,
    pub payload_preview: Option<String>,
    pub weight:          u8,
}

#[derive(Debug, Serialize)]
pub struct SearchSessionResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success"
    pub query:     String,
    pub sort_by:   String,
    pub total:     usize,
    pub truncated: bool,
    pub entries:   Vec<JournalEntry>,
}

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT:     usize = 200;
const PREVIEW_LEN:   usize = 120;

/// Event weight for priority sorting (higher = more important).
fn event_weight(event: &str) -> u8 {
    match event {
        "checkpoint_created"  => 10,
        "stop_attempted"      => 8,
        "write_recorded"      => 7,
        "node_entered"        => 6,
        "node_exited"         => 5,
        "session_started"     => 9,
        "aborted"             => 8,
        _                     => 3,
    }
}

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

    let limit   = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let sort_by = params.sort_by.as_deref().unwrap_or("time").to_string();
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

    // Collect all matching entries (newest first for time sort)
    for line in content.lines().rev() {
        if line.trim().is_empty() { continue; }

        if !line.to_lowercase().contains(&query_lower) { continue; }

        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event   = v.get("event").and_then(|e| e.as_str()).unwrap_or("").to_string();
        let phase   = v.get("phase").and_then(|e| e.as_str()).map(str::to_string);
        let node_id = v.get("node_id").and_then(|e| e.as_str()).map(str::to_string);

        if let Some(ref fe) = params.filter_event {
            if !event.eq_ignore_ascii_case(fe) { continue; }
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

        let ts    = v.get("ts").and_then(|e| e.as_str()).unwrap_or("").to_string();

        // Time range filter (lexicographic ISO 8601 comparison)
        if let Some(ref since) = params.since {
            if !ts.is_empty() && ts.as_str() < since.as_str() { continue; }
        }
        if let Some(ref until) = params.until {
            if !ts.is_empty() && ts.as_str() > until.as_str() { continue; }
        }

        let actor = v.get("actor").and_then(|e| e.as_str()).map(str::to_string);
        let payload_preview = v.get("payload").map(|p| {
            let s = serde_json::to_string(p).unwrap_or_default();
            if s.len() > PREVIEW_LEN { format!("{}…", &s[..PREVIEW_LEN]) } else { s }
        });
        let weight = event_weight(&event);

        matched.push(JournalEntry { ts, event, phase, node_id, actor, payload_preview, weight });
    }

    let total_found = total;

    // Apply sort
    if sort_by == "event_weight" {
        matched.sort_by(|a, b| b.weight.cmp(&a.weight).then(b.ts.cmp(&a.ts)));
    }
    // "time" sort is already newest-first from the reverse iteration

    let truncated = matched.len() > limit;
    matched.truncate(limit);

    Ok(SearchSessionResult {
        kind:      "success".into(),
        query:     params.query,
        sort_by,
        total:     total_found,
        truncated,
        entries:   matched,
    })
}
