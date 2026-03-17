// src/resources/workflow.rs
//
// Four MCP resources backed by direct file I/O:
//   - workflow://session     → .ai/workflow/session.yaml
//   - workflow://task-graph  → .ai/workflow/task-graph.yaml
//   - workflow://journal     → .ai/workflow/journal.jsonl (last 200 lines)
//   - memory://index         → .ai/memory/index.json

use std::path::Path;
use std::fs;

use rmcp::model::*;

/// Known workflow resource URIs.
pub const RESOURCE_SESSION:    &str = "workflow://session";
pub const RESOURCE_TASK_GRAPH: &str = "workflow://task-graph";
pub const RESOURCE_JOURNAL:    &str = "workflow://journal";
pub const RESOURCE_MEMORY:     &str = "memory://index";

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
        _ => return Err(format!("Unknown resource URI: {}", uri)),
    };

    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(content, uri).with_mime_type(mime),
    ]))
}

/// Read a file, returning empty content with a comment if it doesn't exist.
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

/// Read the journal, returning the last 200 lines.
fn read_journal(path: &Path) -> Result<(String, String), String> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let max_lines = 200;
            let start = if lines.len() > max_lines {
                lines.len() - max_lines
            } else {
                0
            };
            let truncated = lines[start..].join("\n");
            Ok((truncated, "application/jsonl".to_string()))
        }
        Err(_) => Ok((String::new(), "application/jsonl".to_string())),
    }
}
