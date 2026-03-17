// src/tools/tdd_evidence.rs
//
// Aggregate TDD evidence from journal.jsonl.
// Returns per-node TDD state progression: red_verified, green_verified, refactor_done.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TddEvidenceParams {
    /// Filter to a specific node_id (default: all nodes).
    pub node_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TddNodeEvidence {
    pub node_id:        String,
    pub red_verified:   bool,
    pub green_verified: bool,
    pub refactor_done:  bool,
    pub files_written:  Vec<String>,
    pub event_count:    usize,
}

#[derive(Debug, Serialize)]
pub struct TddEvidenceResult {
    #[serde(rename = "type")]
    pub kind:  String, // "success" | "empty"
    pub total: usize,
    pub nodes: Vec<TddNodeEvidence>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_tdd_evidence(
    params: TddEvidenceParams,
    workflow_dir: &Path,
) -> Result<TddEvidenceResult, ToolError> {
    let journal_path = workflow_dir.join("journal.jsonl");

    let content = if journal_path.exists() {
        fs::read_to_string(&journal_path)
            .map_err(|e| ToolError::IoError { message: format!("read journal: {e}") })?
    } else {
        return Ok(TddEvidenceResult { kind: "empty".into(), total: 0, nodes: vec![] });
    };

    // Accumulate per-node evidence
    let mut node_map: HashMap<String, TddNodeEvidence> = HashMap::new();

    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event   = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
        let node_id = v.get("node_id").and_then(|e| e.as_str()).unwrap_or("");

        if node_id.is_empty() { continue; }

        // Apply node filter
        if let Some(ref filter) = params.node_id {
            if !node_id.eq_ignore_ascii_case(filter) { continue; }
        }

        let entry = node_map.entry(node_id.to_string()).or_insert_with(|| TddNodeEvidence {
            node_id:        node_id.to_string(),
            red_verified:   false,
            green_verified: false,
            refactor_done:  false,
            files_written:  vec![],
            event_count:    0,
        });

        entry.event_count += 1;

        match event {
            "tdd_state_changed" => {
                let payload = v.get("payload");
                let new_state = payload
                    .and_then(|p| p.get("new_state"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                match new_state {
                    "red_verified"   => entry.red_verified   = true,
                    "green_verified" => entry.green_verified = true,
                    "refactor_done"  => entry.refactor_done  = true,
                    _ => {}
                }
            }
            "write_recorded" => {
                // Also infer TDD state from write_recorded payload tdd_state field
                let payload = v.get("payload");
                if let Some(tdd_state) = payload
                    .and_then(|p| p.get("tdd_state"))
                    .and_then(|s| s.as_str())
                {
                    match tdd_state {
                        "red_verified"   => entry.red_verified   = true,
                        "green_verified" => entry.green_verified = true,
                        "refactor_done"  => entry.refactor_done  = true,
                        _ => {}
                    }
                }
                if let Some(path) = payload
                    .and_then(|p| p.get("path"))
                    .and_then(|s| s.as_str())
                {
                    if !entry.files_written.contains(&path.to_string()) {
                        entry.files_written.push(path.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    let mut nodes: Vec<TddNodeEvidence> = node_map.into_values().collect();
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    let total = nodes.len();

    Ok(TddEvidenceResult {
        kind: if total == 0 { "empty" } else { "success" }.into(),
        total,
        nodes,
    })
}
