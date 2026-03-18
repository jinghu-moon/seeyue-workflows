// src/tools/interaction_mcp.rs — P2-N4: MCP tools for active interactions
//
// Tools:
//   sy_list_interactions  — list pending interaction IDs from interactions/requests/
//   sy_read_interaction   — read a specific request by ID
//   sy_resolve_interaction — write a response file (MCP-driven resolution)
//
// Resource:
//   workflow://interactions/active — reads interactions/active.json

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListInteractionsParams {
    /// Filter by status. Default: "pending".
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadInteractionParams {
    /// Interaction ID to read (e.g. ix-20260318-001).
    pub interaction_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveInteractionParams {
    /// Interaction ID to resolve.
    pub interaction_id: String,
    /// Selected option ID (e.g. "approve", "reject", or free text).
    pub selected_option: String,
    /// Optional comment from the resolver.
    pub comment: Option<String>,
}

// ─── Results ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ListInteractionsResult {
    #[serde(rename = "type")]
    pub kind:    String,
    pub total:   usize,
    pub items:   Vec<InteractionSummary>,
}

#[derive(Debug, Serialize)]
pub struct InteractionSummary {
    pub interaction_id: String,
    pub kind:           String,
    pub status:         String,
    pub title:          String,
    pub created_at:     String,
}

#[derive(Debug, Serialize)]
pub struct ReadInteractionResult {
    #[serde(rename = "type")]
    pub kind:    String,
    pub found:   bool,
    pub data:    Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ResolveInteractionResult {
    #[serde(rename = "type")]
    pub kind:           String,
    pub interaction_id: String,
    pub resolved:       bool,
    pub response_path:  String,
}

// ─── Active interactions resource helper ─────────────────────────────────────

/// Read .ai/workflow/interactions/active.json and return structured JSON.
/// Returns empty/null object if file doesn't exist.
pub fn read_active_interactions(workflow_dir: &Path) -> serde_json::Value {
    let active_path = workflow_dir
        .join("interactions")
        .join("active.json");

    if !active_path.exists() {
        return serde_json::json!({
            "active_id": null,
            "pending_count": 0,
            "blocking_kind": null,
            "blocking_reason": null,
        });
    }

    match fs::read_to_string(&active_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({
            "active_id": null,
            "pending_count": 0,
            "blocking_kind": null,
            "blocking_reason": null,
            "_parse_error": "active.json is not valid JSON",
        })),
        Err(e) => serde_json::json!({
            "active_id": null,
            "pending_count": 0,
            "blocking_kind": null,
            "blocking_reason": null,
            "_read_error": e.to_string(),
        }),
    }
}

// ─── sy_list_interactions ─────────────────────────────────────────────────────

pub fn run_list_interactions(
    params: ListInteractionsParams,
    workflow_dir: &Path,
) -> Result<ListInteractionsResult, ToolError> {
    let requests_dir = workflow_dir.join("interactions").join("requests");
    let filter_status = params.status.as_deref().unwrap_or("pending");

    if !requests_dir.exists() {
        return Ok(ListInteractionsResult {
            kind:  "list".into(),
            total: 0,
            items: vec![],
        });
    }

    let mut items: Vec<InteractionSummary> = vec![];

    let entries = fs::read_dir(&requests_dir)
        .map_err(|e| ToolError::IoError { message: format!("read interactions/requests: {e}") })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let obj: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let status = obj.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if filter_status != "all" && status != filter_status {
            continue;
        }
        items.push(InteractionSummary {
            interaction_id: obj.get("interaction_id").and_then(|v| v.as_str()).unwrap_or("").into(),
            kind:           obj.get("kind").and_then(|v| v.as_str()).unwrap_or("").into(),
            status:         status.into(),
            title:          obj.get("title").and_then(|v| v.as_str()).unwrap_or("").into(),
            created_at:     obj.get("created_at").and_then(|v| v.as_str()).unwrap_or("").into(),
        });
    }

    // Sort by created_at descending
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let total = items.len();

    Ok(ListInteractionsResult { kind: "list".into(), total, items })
}

// ─── sy_read_interaction ──────────────────────────────────────────────────────

pub fn run_read_interaction(
    params: ReadInteractionParams,
    workflow_dir: &Path,
) -> Result<ReadInteractionResult, ToolError> {
    // Validate ID format to prevent path traversal
    if !params.interaction_id.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(ToolError::IoError {
            message: format!("invalid interaction_id: '{}'", params.interaction_id),
        });
    }

    let file_path = workflow_dir
        .join("interactions")
        .join("requests")
        .join(format!("{}.json", params.interaction_id));

    if !file_path.exists() {
        return Ok(ReadInteractionResult {
            kind:  "not_found".into(),
            found: false,
            data:  None,
        });
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| ToolError::IoError { message: format!("read interaction: {e}") })?;
    let data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| ToolError::IoError { message: format!("parse interaction: {e}") })?;

    Ok(ReadInteractionResult {
        kind:  "found".into(),
        found: true,
        data:  Some(data),
    })
}

// ─── sy_resolve_interaction ───────────────────────────────────────────────────

pub fn run_resolve_interaction(
    params: ResolveInteractionParams,
    workflow_dir: &Path,
) -> Result<ResolveInteractionResult, ToolError> {
    // Validate ID
    if !params.interaction_id.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(ToolError::IoError {
            message: format!("invalid interaction_id: '{}'", params.interaction_id),
        });
    }

    // Ensure responses directory exists
    let responses_dir = workflow_dir.join("interactions").join("responses");
    fs::create_dir_all(&responses_dir)
        .map_err(|e| ToolError::IoError { message: format!("create responses dir: {e}") })?;

    let ts = Utc::now().to_rfc3339();
    let response_obj = serde_json::json!({
        "interaction_id":  params.interaction_id,
        "selected_option": params.selected_option,
        "comment":         params.comment,
        "resolved_at":     ts,
        "resolver":        "mcp",
    });

    let response_path = responses_dir.join(format!("{}.json", params.interaction_id));
    let content = serde_json::to_string_pretty(&response_obj)
        .map_err(|e| ToolError::IoError { message: format!("serialize response: {e}") })?;
    fs::write(&response_path, format!("{content}\n"))
        .map_err(|e| ToolError::IoError { message: format!("write response: {e}") })?;

    Ok(ResolveInteractionResult {
        kind:           "resolved".into(),
        interaction_id: params.interaction_id,
        resolved:       true,
        response_path:  response_path.to_string_lossy().into_owned(),
    })
}
