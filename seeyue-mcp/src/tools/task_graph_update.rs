// src/tools/task_graph_update.rs
//
// Update a node's status/notes in .ai/workflow/task-graph.yaml.
// Reads the file as raw text, does targeted field replacement, writes back.

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::workflow::journal::{self, JournalEvent};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TaskGraphUpdateParams {
    /// Node id to update.
    pub node_id: String,
    /// New status value (e.g. "completed", "in_progress", "skipped").
    pub status:  Option<String>,
    /// Notes to attach to the node.
    pub notes:   Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskGraphUpdateResult {
    #[serde(rename = "type")]
    pub kind:    String, // "updated" | "not_found"
    pub node_id: String,
    pub status:  Option<String>,
    pub updated: String,
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

    let content = fs::read_to_string(&path)
        .map_err(|e| ToolError::IoError { message: format!("read task-graph: {e}") })?;

    // Check node exists (simple substring check on id field)
    if !content.contains(&format!("id: {}", params.node_id))
        && !content.contains(&format!("id: '{}'", params.node_id))
        && !content.contains(&format!("id: \"{}\"", params.node_id))
    {
        return Ok(TaskGraphUpdateResult {
            kind:    "not_found".into(),
            node_id: params.node_id,
            status:  params.status,
            updated: Utc::now().to_rfc3339(),
        });
    }

    // Parse as serde_yaml Value for safe field injection
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| ToolError::IoError { message: format!("parse yaml: {e}") })?;

    let updated_at = Utc::now().to_rfc3339();
    let mut found = false;

    // Walk nodes array at root or under phases
    if let Some(nodes) = doc.get_mut("nodes").and_then(|v| v.as_sequence_mut()) {
        for node in nodes.iter_mut() {
            let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id == params.node_id {
                found = true;
                if let Some(ref s) = params.status {
                    node["status"] = serde_yaml::Value::String(s.clone());
                }
                if let Some(ref n) = params.notes {
                    node["notes"] = serde_yaml::Value::String(n.clone());
                }
                node["updated_at"] = serde_yaml::Value::String(updated_at.clone());
                break;
            }
        }
    }

    if !found {
        // Try under phases[].nodes[]
        if let Some(phases) = doc.get_mut("phases").and_then(|v| v.as_sequence_mut()) {
            'outer: for phase in phases.iter_mut() {
                if let Some(nodes) = phase.get_mut("nodes").and_then(|v| v.as_sequence_mut()) {
                    for node in nodes.iter_mut() {
                        let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        if id == params.node_id {
                            found = true;
                            if let Some(ref s) = params.status {
                                node["status"] = serde_yaml::Value::String(s.clone());
                            }
                            if let Some(ref n) = params.notes {
                                node["notes"] = serde_yaml::Value::String(n.clone());
                            }
                            node["updated_at"] = serde_yaml::Value::String(updated_at.clone());
                            break 'outer;
                        }
                    }
                }
            }
        }
    }

    if !found {
        return Ok(TaskGraphUpdateResult {
            kind:    "not_found".into(),
            node_id: params.node_id,
            status:  params.status,
            updated: updated_at,
        });
    }

    let new_content = serde_yaml::to_string(&doc)
        .map_err(|e| ToolError::IoError { message: format!("serialize yaml: {e}") })?;
    fs::write(&path, &new_content)
        .map_err(|e| ToolError::IoError { message: format!("write task-graph: {e}") })?;

    // Journal
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "task_graph_updated".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "node_id": params.node_id,
            "status":  params.status,
            "notes":   params.notes,
        })),
        phase:    None,
        node_id:  Some(params.node_id.clone()),
        run_id:   None,
        ts:       chrono::Utc::now().to_rfc3339(),
        trace_id: None,
    });

    Ok(TaskGraphUpdateResult {
        kind:    "updated".into(),
        node_id: params.node_id,
        status:  params.status,
        updated: updated_at,
    })
}
