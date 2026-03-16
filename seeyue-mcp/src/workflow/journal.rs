// src/workflow/journal.rs
//
// Atomic append to `.ai/workflow/journal.jsonl`.
// Each event is a single JSON line with timestamp, run_id, event type, etc.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;
use serde_json::Value as JsonValue;

// ─── Journal Event ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct JournalEvent {
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default)]
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl JournalEvent {
    /// Create a new journal event with current timestamp.
    pub fn new(event: impl Into<String>, actor: impl Into<String>) -> Self {
        Self {
            ts: Utc::now().to_rfc3339(),
            run_id: None,
            event: event.into(),
            phase: None,
            node_id: None,
            actor: actor.into(),
            payload: None,
            trace_id: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    pub fn with_payload(mut self, payload: JsonValue) -> Self {
        self.payload = Some(payload);
        self
    }
}

// ─── Append ──────────────────────────────────────────────────────────────────

/// Atomically append a journal event to `journal.jsonl`.
///
/// Uses append mode with explicit flush to ensure durability.
/// Creates parent directories if needed.
pub fn append_event(workflow_dir: &Path, event: JournalEvent) -> Result<(), String> {
    let path = workflow_dir.join("journal.jsonl");

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create journal dir: {}", e))?;
    }

    let mut line = serde_json::to_string(&event)
        .map_err(|e| format!("Failed to serialize journal event: {}", e))?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open journal.jsonl: {}", e))?;

    file.write_all(line.as_bytes())
        .map_err(|e| format!("Failed to write journal event: {}", e))?;

    file.flush()
        .map_err(|e| format!("Failed to flush journal: {}", e))?;

    Ok(())
}

/// Read the last N lines from journal.jsonl.
#[allow(dead_code)]
pub fn read_recent(workflow_dir: &Path, max_lines: usize) -> Result<String, String> {
    let path = workflow_dir.join("journal.jsonl");

    match fs::read_to_string(&path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > max_lines {
                lines.len() - max_lines
            } else {
                0
            };
            Ok(lines[start..].join("\n"))
        }
        Err(_) => Ok(String::new()),
    }
}
