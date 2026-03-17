// src/tools/task_graph_update.rs
//
// Update node status/notes in .ai/workflow/task-graph.yaml.
// Supports single-node and batch update modes.

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::workflow::journal::{self, JournalEvent};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct NodeUpdate {
    pub node_id: String,
    pub status:  Option<String>,
    pub notes:   Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskGraphUpdateParams {
    /// Node id to update (single-node mode; omit when using `nodes`).
    pub node_id: Option<String>,
    /// New status value (e.g. "completed", "in_progress", "skipped").
    pub status:  Option<String>,
    /// Notes to attach to the node.
    pub notes:   Option<String>,
    /// Batch mode: list of {node_id, status?, notes?} updates.
    pub nodes:   Option<Vec<NodeUpdate>>,
}

#[derive(Debug, Serialize)]
pub struct NodeUpdateResult {
    pub node_id: String,
    pub kind:    String, // "updated" | "not_found"
    pub status:  Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskGraphUpdateResult {
    #[serde(rename = "type")]
    pub kind:    String, // "updated" | "not_found" | "batch"
    /// Populated in single-node mode.
    pub node_id: Option<String>,
    pub status:  Option<String>,
    pub updated: String,
    /// Populated in batch mode.
    pub results: Vec<NodeUpdateResult>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_task_graph_update(
    params: TaskGraphUpdateParams,
    workflow_dir: &Path,
) -> Result<TaskGraphUpdateResult, ToolError> {
    let path = workflow_dir.join("task-graph.yaml");

    if !path.exists() {
        return Err(ToolError::IoError {
            message: "task-graph.yaml not found — initialize workflow first.".into(),
        });
    }

    // Build canonical list of updates
    let updates: Vec<NodeUpdate> = match params.nodes {
        Some(batch) => batch,
        None => {
            let node_id = params.node_id.ok_or_else(|| ToolError::MissingParameter {
                missing: "node_id".into(),
                hint:    "Provide node_id or nodes[] for batch mode.".into(),
            })?;
            vec![NodeUpdate { node_id, status: params.status, notes: params.notes }]
        }
    };

    let is_batch = updates.len() > 1;
    let content  = fs::read_to_string(&path)
        .map_err(|e| ToolError::IoError { message: format!("read task-graph: {e}") })?;
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| ToolError::IoError { message: format!("parse yaml: {e}") })?;

    let updated_at = Utc::now().to_rfc3339();
    let mut results: Vec<NodeUpdateResult> = Vec::new();

    for upd in &updates {
        let found = apply_update(&mut doc, upd, &updated_at);
        results.push(NodeUpdateResult {
            node_id: upd.node_id.clone(),
            kind:    if found { "updated" } else { "not_found" }.into(),
            status:  upd.status.clone(),
        });
    }

    let new_content = serde_yaml::to_string(&doc)
        .map_err(|e| ToolError::IoError { message: format!("serialize yaml: {e}") })?;
    fs::write(&path, &new_content)
        .map_err(|e| ToolError::IoError { message: format!("write task-graph: {e}") })?;

    // Journal — one event covering all updates
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "task_graph_updated".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({ "updates": results
            .iter()
            .map(|r| serde_json::json!({"node_id": r.node_id, "status": r.status, "kind": r.kind}))
            .collect::<Vec<_>>() })),
        phase:    None,
        node_id:  if is_batch { None } else { results.first().map(|r| r.node_id.clone()) },
        run_id:   None,
        ts:       updated_at.clone(),
        trace_id: None,
    });

    if is_batch {
        Ok(TaskGraphUpdateResult {
            kind:    "batch".into(),
            node_id: None,
            status:  None,
            updated: updated_at,
            results,
        })
    } else {
        let r = results.into_iter().next().unwrap();
        Ok(TaskGraphUpdateResult {
            kind:    r.kind,
            node_id: Some(r.node_id),
            status:  r.status,
            updated: updated_at,
            results: vec![],
        })
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Apply a single NodeUpdate to the parsed YAML doc. Returns true if node was found.
fn apply_update(doc: &mut serde_yaml::Value, upd: &NodeUpdate, updated_at: &str) -> bool {
    if try_update_in_sequence(
        doc.get_mut("nodes").and_then(|v| v.as_sequence_mut()),
        upd, updated_at,
    ) {
        return true;
    }
    if let Some(phases) = doc.get_mut("phases").and_then(|v| v.as_sequence_mut()) {
        for phase in phases.iter_mut() {
            if try_update_in_sequence(
                phase.get_mut("nodes").and_then(|v| v.as_sequence_mut()),
                upd, updated_at,
            ) {
                return true;
            }
        }
    }
    false
}

fn try_update_in_sequence(
    seq: Option<&mut Vec<serde_yaml::Value>>,
    upd: &NodeUpdate,
    updated_at: &str,
) -> bool {
    let nodes = match seq { Some(s) => s, None => return false };
    for node in nodes.iter_mut() {
        let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id == upd.node_id {
            if let Some(ref s) = upd.status {
                node["status"] = serde_yaml::Value::String(s.clone());
            }
            if let Some(ref n) = upd.notes {
                node["notes"] = serde_yaml::Value::String(n.clone());
            }
            node["updated_at"] = serde_yaml::Value::String(updated_at.to_string());
            return true;
        }
    }
    false
}
