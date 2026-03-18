// src/resources/workflow.rs
//
// MCP resources backed by direct file I/O:
//   - workflow://session     → .ai/workflow/session.yaml
//   - workflow://task-graph  → .ai/workflow/task-graph.yaml
//   - workflow://journal     → .ai/workflow/journal.jsonl (last 200 lines)
//   - memory://index         → .ai/memory/index.json
//   - workflow://dashboard   → aggregated session + approvals + budget + recent events

use std::path::Path;
use std::fs;

use rmcp::model::*;

/// Known workflow resource URIs.
pub const RESOURCE_SESSION:    &str = "workflow://session";
pub const RESOURCE_TASK_GRAPH: &str = "workflow://task-graph";
pub const RESOURCE_JOURNAL:    &str = "workflow://journal";
pub const RESOURCE_MEMORY:     &str = "memory://index";
pub const RESOURCE_DASHBOARD:  &str = "workflow://dashboard";
pub const RESOURCE_QUESTIONS:  &str = "workflow://questions";
pub const RESOURCE_INPUTS:     &str = "workflow://inputs";
pub const RESOURCE_ERRORS:     &str = "workspace://errors";
pub const RESOURCE_INTERACTIONS_ACTIVE: &str = "workflow://interactions/active";

/// List all available workflow resources as rmcp Resource objects.
pub fn list_resources() -> Vec<Resource> {
    vec![
        RawResource::new(RESOURCE_SESSION, "Workflow Session State")
            .with_description(
                "Current workflow session state (phase, node, loop budget, approvals, recovery). \
                 Source: .ai/workflow/session.yaml",
            )
            .with_mime_type("text/yaml")
            .no_annotation(),
        RawResource::new(RESOURCE_TASK_GRAPH, "Workflow Task Graph")
            .with_description(
                "Task graph with phases, nodes, and dependencies. \
                 Source: .ai/workflow/task-graph.yaml",
            )
            .with_mime_type("text/yaml")
            .no_annotation(),
        RawResource::new(RESOURCE_JOURNAL, "Workflow Journal")
            .with_description(
                "Append-only event journal (JSONL). Shows recent events. \
                 Source: .ai/workflow/journal.jsonl",
            )
            .with_mime_type("application/jsonl")
            .no_annotation(),
        RawResource::new(RESOURCE_MEMORY, "Memory Index")
            .with_description(
                "Cross-session memory index. Lists all persisted memory keys, tags, and previews. \
                 Source: .ai/memory/index.json",
            )
            .with_mime_type("application/json")
            .no_annotation(),
        RawResource::new(RESOURCE_DASHBOARD, "Workflow Dashboard")
            .with_description(
                "Aggregated snapshot: active node, pending approvals, pending questions, \
                 pending inputs, budget info, and last 5 journal events. \
                 Read once per turn to replace multiple separate status queries.",
            )
            .with_mime_type("application/json")
            .no_annotation(),
        RawResource::new(RESOURCE_QUESTIONS, "Pending Questions")
            .with_description(
                "Questions posted by sy_ask_user awaiting user response. \
                 Source: .ai/workflow/questions.jsonl",
            )
            .with_mime_type("application/jsonl")
            .no_annotation(),
        RawResource::new(RESOURCE_INPUTS, "Pending Input Requests")
            .with_description(
                "Structured input requests posted by sy_input_request. \
                 Source: .ai/workflow/input_requests.jsonl",
            )
            .with_mime_type("application/jsonl")
            .no_annotation(),
        RawResource::new(RESOURCE_ERRORS, "Workspace Errors")
            .with_description(
                "Aggregated lint and type-check errors from the last run. \
                 Source: .ai/workflow/errors.json (written by lint_file/type_check tools)",
            )
            .with_mime_type("application/json")
            .no_annotation(),
        RawResource::new(RESOURCE_INTERACTIONS_ACTIVE, "Active Interactions")
            .with_description(
                "Current active interaction index: active_id, pending_count, blocking_kind, blocking_reason. \
                 Source: .ai/workflow/interactions/active.json",
            )
            .with_mime_type("application/json")
            .no_annotation(),
    ]
}

/// Read a workflow resource by URI. Returns ReadResourceResult for rmcp.
pub fn read_resource(
    uri: &str,
    workflow_dir: &Path,
    workspace: &Path,
) -> Result<ReadResourceResult, String> {
    let (content, mime) = match uri {
        RESOURCE_SESSION => {
            let path = workflow_dir.join("session.yaml");
            read_file_or_empty(&path, "text/yaml")?
        }
        RESOURCE_TASK_GRAPH => {
            let path = workflow_dir.join("task-graph.yaml");
            read_file_or_empty(&path, "text/yaml")?
        }
        RESOURCE_JOURNAL => {
            let path = workflow_dir.join("journal.jsonl");
            read_journal(&path)?
        }
        RESOURCE_MEMORY => {
            let path = workspace.join(".ai/memory/index.json");
            read_file_or_empty(&path, "application/json")?
        }
        RESOURCE_DASHBOARD => {
            build_dashboard(workflow_dir)?
        }
        RESOURCE_QUESTIONS => {
            let path = workflow_dir.join("questions.jsonl");
            read_file_or_empty(&path, "application/jsonl")?
        }
        RESOURCE_INPUTS => {
            let path = workflow_dir.join("input_requests.jsonl");
            read_file_or_empty(&path, "application/jsonl")?
        }
        RESOURCE_ERRORS => {
            let path = workflow_dir.join("errors.json");
            read_file_or_empty(&path, "application/json")?
        }
        RESOURCE_INTERACTIONS_ACTIVE => {
            let data = crate::tools::interaction_mcp::read_active_interactions(workflow_dir);
            let json = serde_json::to_string_pretty(&data)
                .map_err(|e| format!("serialize interactions/active: {e}"))?;
            (json, "application/json".to_string())
        }
        _ => return Err(format!("Unknown resource URI: {}", uri)),
    };

    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(content, uri).with_mime_type(mime),
    ]))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn read_file_or_empty(path: &Path, mime: &str) -> Result<(String, String), String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok((content, mime.to_string())),
        Err(_) => Ok((
            format!(
                "# File not found: {}\n# Initialize workflow to create this file.",
                path.display()
            ),
            mime.to_string(),
        )),
    }
}

fn read_journal(path: &Path) -> Result<(String, String), String> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let max_lines = 200;
            let start = if lines.len() > max_lines { lines.len() - max_lines } else { 0 };
            Ok((lines[start..].join("\n"), "application/jsonl".to_string()))
        }
        Err(_) => Ok((String::new(), "application/jsonl".to_string())),
    }
}

/// Build the dashboard JSON aggregating key workflow state.
fn build_dashboard(workflow_dir: &Path) -> Result<(String, String), String> {
    // Session snapshot (YAML → Value)
    let session_val: serde_json::Value = {
        let path = workflow_dir.join("session.yaml");
        match fs::read_to_string(&path) {
            Ok(s) => serde_yaml::from_str(&s).unwrap_or(serde_json::Value::Null),
            Err(_) => serde_json::Value::Null,
        }
    };

    let pending_approvals = count_pending_jsonl(
        &workflow_dir.join("approvals.jsonl"), "status", "pending");
    let pending_questions = count_pending_jsonl(
        &workflow_dir.join("questions.jsonl"), "status", "pending");
    let pending_inputs = count_pending_jsonl(
        &workflow_dir.join("input_requests.jsonl"), "status", "pending");

    // Last 5 journal events
    let recent_events: Vec<serde_json::Value> = {
        let path = workflow_dir.join("journal.jsonl");
        match fs::read_to_string(&path) {
            Ok(content) => {
                let mut events: Vec<serde_json::Value> = content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                let start = if events.len() > 5 { events.len() - 5 } else { 0 };
                events.split_off(start)
            }
            Err(_) => vec![],
        }
    };

    let dashboard = serde_json::json!({
        "active_node":       session_val.get("node").and_then(|n| n.get("name")),
        "active_phase":      session_val.get("phase").and_then(|p| p.get("name")),
        "loop_count":        session_val.get("loop_count"),
        "budget_exceeded":   session_val.get("budget_exceeded"),
        "restore_pending":   session_val.get("restore_pending"),
        "pending_approvals": pending_approvals,
        "pending_questions": pending_questions,
        "pending_inputs":    pending_inputs,
        "recent_events":     recent_events,
    });

    let json = serde_json::to_string_pretty(&dashboard)
        .map_err(|e| format!("serialize dashboard: {e}"))?;
    Ok((json, "application/json".to_string()))
}

/// Count records in a JSONL file where `field == value` (last-record-per-id wins).
fn count_pending_jsonl(path: &std::path::Path, field: &str, value: &str) -> usize {
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut map: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            let key = v.as_object()
                .and_then(|o| o.iter().find(|(k, _)| k.ends_with("_id")))
                .and_then(|(_, v)| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| line.to_string());
            map.insert(key, v);
        }
    }
    map.values()
        .filter(|v| v.get(field).and_then(|f| f.as_str()) == Some(value))
        .count()
}
