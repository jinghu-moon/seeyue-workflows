// src/tools/progress_report.rs
//
// Generate a human-readable progress report for the current workflow phase.
// Aggregates: completed/total nodes, files written, key events.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::state;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ProgressReportParams {
    /// Filter to a specific phase id/name (default: current phase).
    pub phase:  Option<String>,
    /// If true, also send a Windows Toast with the summary line.
    pub notify: bool,
}

#[derive(Debug, Serialize)]
pub struct NodeProgress {
    pub id:     Option<String>,
    pub name:   Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProgressReportResult {
    #[serde(rename = "type")]
    pub kind:            String, // "success" | "no_session"
    pub phase:           Option<String>,
    pub phase_status:    Option<String>,
    pub nodes_total:     usize,
    pub nodes_completed: usize,
    pub nodes_pending:   usize,
    pub files_written:   Vec<String>,
    pub event_counts:    HashMap<String, usize>,
    pub active_node:     Option<String>,
    pub summary:         String,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_progress_report(
    params: ProgressReportParams,
    workflow_dir: &Path,
) -> Result<ProgressReportResult, ToolError> {
    if !workflow_dir.join("session.yaml").exists() {
        return Ok(ProgressReportResult {
            kind:            "no_session".into(),
            phase:           None,
            phase_status:    None,
            nodes_total:     0,
            nodes_completed: 0,
            nodes_pending:   0,
            files_written:   vec![],
            event_counts:    HashMap::new(),
            active_node:     None,
            summary:         "No active session.".into(),
        });
    }

    let session = state::load_session(workflow_dir);
    let phase_id = params.phase
        .or_else(|| session.phase.id.clone())
        .or_else(|| session.phase.name.clone());

    // Read task-graph for node counts
    let (nodes_total, nodes_completed, nodes_pending, node_list) =
        read_node_stats(workflow_dir, phase_id.as_deref());

    // Scan journal for files written and event counts
    let (files_written, event_counts) = scan_journal(workflow_dir, &session.run_id);

    let active_node = session.node.name
        .or(session.node.id)
        .map(|n| format!("{} ({})", n, session.node.status.as_deref().unwrap_or("unknown")));

    let pct = if nodes_total > 0 {
        nodes_completed * 100 / nodes_total
    } else { 0 };

    let summary = format!(
        "Phase '{}' — {}/{} nodes completed ({}%). Active: {}. Files written: {}.",
        phase_id.as_deref().unwrap_or("unknown"),
        nodes_completed, nodes_total, pct,
        active_node.as_deref().unwrap_or("none"),
        files_written.len(),
    );

    let _ = node_list;

    let result = ProgressReportResult {
        kind:            "success".into(),
        phase:           phase_id,
        phase_status:    session.phase.status,
        nodes_total,
        nodes_completed,
        nodes_pending,
        files_written,
        event_counts,
        active_node,
        summary:         summary.clone(),
    };

    if params.notify {
        let level = if nodes_pending == 0 { NotifyLevel::Milestone } else { NotifyLevel::Info };
        win_notify::send_toast("seeyue-mcp [progress]", &summary, level);
    }

    Ok(result)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn read_node_stats(
    workflow_dir: &Path,
    _phase_filter: Option<&str>,
) -> (usize, usize, usize, Vec<NodeProgress>) {
    let path = workflow_dir.join("task-graph.yaml");
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return (0, 0, 0, vec![]),
    };
    let doc: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return (0, 0, 0, vec![]),
    };

    let mut nodes: Vec<NodeProgress> = Vec::new();

    // Collect from root nodes[] or phases[].nodes[]
    if let Some(arr) = doc.get("nodes").and_then(|v| v.as_sequence()) {
        collect_nodes(arr, &mut nodes);
    }
    if let Some(phases) = doc.get("phases").and_then(|v| v.as_sequence()) {
        for phase in phases {
            if let Some(arr) = phase.get("nodes").and_then(|v| v.as_sequence()) {
                collect_nodes(arr, &mut nodes);
            }
        }
    }

    let total     = nodes.len();
    let completed = nodes.iter().filter(|n| {
        n.status.as_deref().map(|s| matches!(s, "completed" | "done" | "skipped")).unwrap_or(false)
    }).count();
    let pending = total - completed;
    (total, completed, pending, nodes)
}

fn collect_nodes(arr: &[serde_yaml::Value], out: &mut Vec<NodeProgress>) {
    for node in arr {
        out.push(NodeProgress {
            id:     node.get("id").and_then(|v| v.as_str()).map(str::to_string),
            name:   node.get("name").and_then(|v| v.as_str()).map(str::to_string),
            status: node.get("status").and_then(|v| v.as_str()).map(str::to_string),
        });
    }
}

fn scan_journal(
    workflow_dir: &Path,
    run_id: &Option<String>,
) -> (Vec<String>, HashMap<String, usize>) {
    let path = workflow_dir.join("journal.jsonl");
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return (vec![], HashMap::new()),
    };

    let mut files: Vec<String> = Vec::new();
    let mut seen_files: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut counts: HashMap<String, usize> = HashMap::new();

    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };

        // Filter by run_id if available
        if let Some(ref rid) = run_id {
            let ev_run = v.get("run_id").and_then(|r| r.as_str()).unwrap_or("");
            if !ev_run.is_empty() && ev_run != rid { continue; }
        }

        let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
        *counts.entry(event.to_string()).or_insert(0) += 1;

        if event == "write_recorded" {
            if let Some(path_str) = v.get("payload")
                .and_then(|p| p.get("path"))
                .and_then(|p| p.as_str())
            {
                if seen_files.insert(path_str.to_string()) {
                    files.push(path_str.to_string());
                }
            }
        }
    }

    (files, counts)
}
