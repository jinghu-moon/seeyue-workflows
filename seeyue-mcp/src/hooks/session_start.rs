// src/hooks/session_start.rs
//
// SessionStart handler: bootstrap workflow context injection.
//
// Reads git state and workflow session, builds the <SY-BOOTSTRAP> directive,
// and emits `additional_context` for Claude Code to inject into the system prompt.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::hooks::protocol::{HookInput, emit_allow_with_extra};
use crate::workflow::state::SessionState;

/// Handle the SessionStart hook event.
pub fn handle(input: &HookInput, _workflow_dir: &Path, session: &SessionState) -> ! {
    let cwd = input.resolve_cwd();

    let mut extras: Vec<String> = Vec::new();

    // 1. Git context
    if let Some(git) = git_context(&cwd) {
        extras.push(git);
    }

    // 2. Workflow context
    if let Some(wf) = workflow_context(session) {
        extras.push(wf);
    }

    // 3. Index existence check
    let index_path = Path::new(&cwd).join(".ai/index.json");
    if !index_path.exists() {
        extras.push(
            "INDEX: .ai/index.json not found — run `/init` before any development task."
                .to_string(),
        );
    }

    // 4. Build bootstrap block
    let bootstrap = build_bootstrap(&extras);

    // 5. Emit allow with additional_context
    let mut extra = HashMap::new();
    extra.insert(
        "additional_context".to_string(),
        Value::String(bootstrap.clone()),
    );
    extra.insert(
        "hookSpecificOutput".to_string(),
        serde_json::json!({
            "hookEventName": "SessionStart",
            "additionalContext": bootstrap,
        }),
    );

    emit_allow_with_extra("session_start", extra)
}

/// Build the <SY-BOOTSTRAP> directive block.
fn build_bootstrap(extras: &[String]) -> String {
    let mut lines = vec![
        "<SY-BOOTSTRAP>".to_string(),
        "If there is even a 1% chance a sy-* skill applies, invoke the relevant skill first."
            .to_string(),
        "Route via `sy-workflow`. Load baseline constraints via `sy-constraints`.".to_string(),
        "Load child constraint skills minimally — baseline + at most 2 task-specific children per turn".to_string(),
        "unless an incident or security escalation is active.".to_string(),
        "Hooks enforce hard guards (dangerous commands, secrets, completion claims).".to_string(),
        "Do NOT implement first and backfill constraints later. Constraints are pre-conditions."
            .to_string(),
        "</SY-BOOTSTRAP>".to_string(),
    ];

    for extra in extras {
        if !extra.is_empty() {
            lines.push(extra.clone());
        }
    }

    lines.join("\n")
}

/// Collect git branch and dirty file count.
fn git_context(cwd: &str) -> Option<String> {
    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(cwd)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .count()
        })
        .unwrap_or(0);

    Some(format!("GIT: branch={}  dirty_files={}", branch, dirty))
}

/// Build workflow context string from session state.
fn workflow_context(session: &SessionState) -> Option<String> {
    let phase = session.phase.name.as_deref().or(session.phase.id.as_deref())?;
    let status = session.phase.status.as_deref().unwrap_or("");

    // Skip if status indicates done/stale
    if status == "completed" || status == "done" {
        return None;
    }

    let node_name = session
        .node
        .name
        .as_deref()
        .or(session.node.id.as_deref())
        .unwrap_or("(unknown)");

    let lines = vec![
        "ACTIVE WORKFLOW:".to_string(),
        format!(
            "  phase={}  node={}  status={}",
            phase, node_name, status
        ),
        "  Run `工作流 继续` to resume or `工作流 状态` to inspect.".to_string(),
    ];

    Some(lines.join("\n"))
}
