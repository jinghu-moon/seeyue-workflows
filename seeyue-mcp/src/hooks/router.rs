// src/hooks/router.rs
//
// Hook event router: dispatches hook_event to the appropriate handler.
//
// PreToolUse:Bash, PreToolUse:Write|Edit, PostToolUse:Write|Edit, and Stop
// are handled inline (they directly call P1 PolicyEngine methods).
// SessionStart, UserPromptSubmit, and PostToolUse:Bash have dedicated modules.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::json;

use crate::hooks::protocol::{HookInput, emit_allow, emit_result};
use crate::hooks::{posttool_bash, prompt_refresh, session_start};
use crate::policy::evaluator::PolicyEngine;
use crate::tools::compact_journal::{CompactJournalParams, run_compact_journal};
use crate::workflow::journal;
use crate::workflow::state::{self, SessionState};

const AUTO_FLUSH_THRESHOLD: usize = 150;
const AUTO_FLUSH_RETAIN:    usize = 100;

/// Dispatch a hook event to the appropriate handler.
///
/// This is the main entry point called by `sy-hook.rs` after setup.
pub fn dispatch(
    hook_event: &str,
    input: &HookInput,
    engine: &PolicyEngine,
    session: &SessionState,
    workflow_dir: &Path,
) -> ! {
    match hook_event {
        "SessionStart" => {
            session_start::handle(input, workflow_dir, session);
        }

        "UserPromptSubmit" => {
            prompt_refresh::handle(input, workflow_dir, session);
        }

        // ── PreToolUse:Bash ─────────────────────────────────────────────
        "PreToolUse:Bash" => {
            let cmd = input
                .tool_input_str("command")
                .or_else(|| input.tool_input_str("cmd"))
                .unwrap_or_default();

            if cmd.trim().is_empty() {
                emit_allow("empty_command");
            }

            let result = engine.check_bash(&cmd, session);
            emit_result(result, Some(build_ctx_bash(session)));
        }

        // ── PreToolUse:Write|Edit ───────────────────────────────────────
        "PreToolUse:Write" | "PreToolUse:Edit" | "PreToolUse:Write|Edit" => {
            let path = input
                .tool_input_str("file_path")
                .or_else(|| input.tool_input_str("path"))
                .unwrap_or_default();

            if path.trim().is_empty() {
                emit_allow("empty_file_path");
            }

            let result = engine.check_write(&path, session);

            // Pre-destructive checkpoint: if allowed and target exists, write a .sy-bak
            // snapshot so the overwrite is recoverable without the SQLite checkpoint store.
            if result.verdict == crate::policy::types::Verdict::Allow {
                let cwd = input.resolve_cwd();
                let full_path = std::path::Path::new(&cwd).join(&path);
                if full_path.exists() {
                    let bak_path = full_path.with_extension(
                        format!(
                            "{}.sy-bak",
                            full_path.extension().and_then(|e| e.to_str()).unwrap_or("")
                        )
                    );
                    // Non-fatal: backup failure must not block the write.
                    if let Ok(content) = fs::read(&full_path) {
                        let _ = fs::write(&bak_path, &content);

                        // Record pre-write backup event in journal.
                        let run_id  = session.run_id.as_deref().unwrap_or("").to_string();
                        let node_id = session.node.id.as_deref()
                            .or(session.node.name.as_deref()).unwrap_or("").to_string();
                        let phase_id = session.phase.id.as_deref()
                            .or(session.phase.name.as_deref()).unwrap_or("none").to_string();
                        if !run_id.is_empty() {
                            let evt = journal::JournalEvent::new("pre_write_backup", "hook")
                                .with_run_id(&run_id)
                                .with_phase(&phase_id)
                                .with_node_id(&node_id)
                                .with_payload(serde_json::json!({
                                    "original": path,
                                    "backup": bak_path.display().to_string(),
                                    "size_bytes": content.len(),
                                }));
                            let _ = journal::append_event(workflow_dir, evt);
                        }
                    }
                }
            }

            emit_result(result, Some(build_ctx_write(session)));
        }

        // ── PostToolUse:Write|Edit ──────────────────────────────────────
        "PostToolUse:Write" | "PostToolUse:Edit" | "PostToolUse:Write|Edit" => {
            handle_posttool_write(input, workflow_dir, session);
        }

        // ── PostToolUse:Bash ────────────────────────────────────────────
        "PostToolUse:Bash" => {
            posttool_bash::handle(input, workflow_dir, session);
        }

        // ── Stop ────────────────────────────────────────────────────────
        "Stop" => {
            let result = engine.check_stop(session);
            emit_result(result, Some(build_ctx_stop(session)));
        }

        // ── Unknown event → fail-open ───────────────────────────────────
        _ => {
            emit_allow(&format!("unknown_hook_event: {}", hook_event));
        }
    }
}

// ─── PostToolUse:Write|Edit (inline handler) ────────────────────────────────

/// Handle PostToolUse:Write|Edit: record audit trail and journal event.
fn handle_posttool_write(
    input: &HookInput,
    workflow_dir: &Path,
    session: &SessionState,
) -> ! {
    let cwd = input.resolve_cwd();
    let file_path = input
        .tool_input_str("file_path")
        .or_else(|| input.tool_input_str("path"))
        .unwrap_or_default();
    let tool_name = input
        .tool_name
        .as_deref()
        .unwrap_or("Write")
        .to_string();

    // Append audit entry
    let audit_path = Path::new(&cwd).join(".ai/workflow/audit.jsonl");
    append_audit(&audit_path, &json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "event": "PostToolUse",
        "tool": tool_name,
        "file": file_path,
    }));

    // Record journal event if run_id exists
    let run_id  = session.run_id.as_deref().unwrap_or("").to_string();
    let node_id  = session.node.id.as_deref()
        .or(session.node.name.as_deref()).unwrap_or("").to_string();
    let phase_id = session.phase.id.as_deref()
        .or(session.phase.name.as_deref()).unwrap_or("none").to_string();
    let scope_drift = check_scope_drift(session, &file_path, &cwd);

    // Use shared helper — single source of truth for write_recorded schema.
    let checkpoint_label = session.recovery.last_checkpoint_id.clone();

    // Compute before hash (from .sy-bak) and after hash (current file)
    let full_path = Path::new(&cwd).join(&file_path);
    let before_hash = {
        let bak = full_path.with_extension(
            format!("{}.sy-bak", full_path.extension().and_then(|e| e.to_str()).unwrap_or(""))
        );
        if bak.exists() { fs::read(&bak).ok().map(|b| hex_sha256(&b)) } else { None }
    };
    let after_hash = if full_path.exists() {
        fs::read(&full_path).ok().map(|b| hex_sha256(&b))
    } else {
        None
    };

    let _ = journal::record_write_evidence(journal::WriteEvidenceParams {
        workflow_dir,
        run_id:           &run_id,
        phase:            &phase_id,
        node_id:          &node_id,
        tool:             &tool_name,
        path:             &file_path,
        lines_changed:    None,
        outcome:          "success",
        checkpoint_label: checkpoint_label.as_deref(),
        syntax_valid:     None,
        scope_drift,
        before_hash,
        after_hash,
    });

    // Auto-flush: compact journal if it exceeds threshold
    if journal::count_lines(workflow_dir) > AUTO_FLUSH_THRESHOLD {
        let _ = run_compact_journal(
            CompactJournalParams { max_entries: Some(AUTO_FLUSH_RETAIN), summarize: false },
            workflow_dir,
        );
    }

    emit_allow("allow_posttool_write")
}

/// Check if file is outside the node's target scope.
fn check_scope_drift(session: &SessionState, file_path: &str, _cwd: &str) -> bool {
    // Skip evidence files
    let is_evidence = file_path.ends_with(".md")
        || file_path.ends_with(".jsonl")
        || file_path.ends_with(".json")
        || file_path.ends_with(".yaml")
        || file_path.ends_with(".yml")
        || file_path.ends_with(".txt")
        || file_path.ends_with(".log");

    if is_evidence || file_path.is_empty() {
        return false;
    }

    // Check phase is execute
    let phase = session.phase.name.as_deref().or(session.phase.id.as_deref());
    if phase != Some("execute") {
        return false;
    }

    // Check target
    if let Some(targets) = &session.node.target {
        let normalized = file_path.replace('\\', "/");
        let in_scope = targets.iter().any(|t| {
            let t_norm = t.replace('\\', "/");
            normalized.starts_with(&t_norm) || t_norm.contains('*')
        });
        return !in_scope;
    }

    false
}

/// Append a JSON line to an audit log file.
fn append_audit(path: &Path, entry: &serde_json::Value) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut line) = serde_json::to_string(entry) {
        line.push('\n');
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())
            });
    }
}

// ─── Session context injector (Phase 4 + Disclosure) ────────────────────────
//
// Disclosure principle (borrowed from nocturne_memory):
// Each call path receives only the fields it actually needs for decision-making.
// This reduces per-verdict payload size and makes each field's provenance clear.
//
//   bash path  → budget + loop guard fields
//   write path → TDD + recovery fields
//   stop path  → approval + checkpoint + phase/node fields

fn make_ctx(ctx: serde_json::Value) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();
    map.insert("session_context".to_string(), ctx);
    map
}

/// Bash path: budget guard + restore gate.
fn build_ctx_bash(session: &SessionState) -> HashMap<String, serde_json::Value> {
    let budget_exceeded = state::check_loop_budget(session);
    make_ctx(json!({
        "run_id":          session.run_id,
        "budget_exceeded": budget_exceeded,
        "restore_pending": session.recovery.status.as_deref() == Some("restore_pending"),
        "loop_count":      session.loop_budget.consumed_nodes
                               .or(session.loop_budget.used)
                               .unwrap_or(0),
    }))
}

/// Write path: TDD gate + recovery gate.
fn build_ctx_write(session: &SessionState) -> HashMap<String, serde_json::Value> {
    make_ctx(json!({
        "run_id":              session.run_id,
        "tdd_state":           session.node.tdd_state,
        "restore_pending":     session.recovery.status.as_deref() == Some("restore_pending"),
        "last_checkpoint_id":  session.recovery.last_checkpoint_id,
    }))
}

/// Stop path: approval count + phase/node for resume.
fn build_ctx_stop(session: &SessionState) -> HashMap<String, serde_json::Value> {
    make_ctx(json!({
        "run_id":              session.run_id,
        "phase":               session.phase.id.as_deref().or(session.phase.name.as_deref()),
        "node_id":             session.node.id.as_deref().or(session.node.name.as_deref()),
        "pending_approvals":   session.approvals.pending.as_ref().map(|v| v.len()).unwrap_or(0),
        "last_checkpoint_id":  session.recovery.last_checkpoint_id,
    }))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Compute SHA-256 hex digest of a byte slice.
fn hex_sha256(data: &[u8]) -> String {
    crate::workflow::journal::hex_sha256(data)
}
