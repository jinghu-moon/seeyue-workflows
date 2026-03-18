// src/tools/posttool_bash.rs
//
// sy_posttool_bash: Explicit tool version of the PostToolUse:Bash hook.
// Captures bash command execution evidence to journal and verify-staging.
// Use after running commands via run_command to record exit code + signals.

use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::hooks::verify_staging;
use crate::workflow::journal::{self, JournalEvent};

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PosttoolBashParams {
    /// The command that was executed.
    pub command:   String,
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// stdout output (truncated to 4KB internally).
    pub stdout:    Option<String>,
    /// stderr output (truncated to 2KB internally).
    pub stderr:    Option<String>,
    /// Node id context for TDD evidence tagging.
    pub node_id:   Option<String>,
    /// Run id context.
    pub run_id:    Option<String>,
    /// Phase context.
    pub phase:     Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PosttoolBashResult {
    #[serde(rename = "type")]
    pub kind:         String, // "recorded"
    pub command:      String,
    pub exit_code:    i32,
    pub phase:        String, // verify phase: build | test | lint | typecheck | security | other
    pub key_signal:   Option<String>,
    pub tdd_evidence: Option<String>, // "red" | "green" | null
    pub recorded:     bool,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_posttool_bash(
    params: PosttoolBashParams,
    workflow_dir: &Path,
) -> Result<PosttoolBashResult, ToolError> {
    if params.command.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "command".into(),
            hint:    "Provide the command that was executed.".into(),
        });
    }

    let stdout    = params.stdout.as_deref().unwrap_or("");
    let stderr    = params.stderr.as_deref().unwrap_or("");
    let exit_code = params.exit_code;

    let phase      = verify_staging::classify_phase(&params.command)
        .unwrap_or("other");
    let key_signal = verify_staging::extract_key_signal(stdout, stderr, exit_code as i64);
    let normalized = verify_staging::normalize_command(&params.command);

    // Detect TDD evidence
    let tdd_evidence = detect_tdd_evidence(stdout, stderr, exit_code, phase);

    // Truncate for storage
    let stdout_stored = truncate(stdout, 4096);
    let stderr_stored = truncate(stderr, 2048);

    // Journal event
    let ts = Utc::now().to_rfc3339();
    let evt = JournalEvent {
        event:   "bash_evidence".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "command":     normalized,
            "exit_code":   exit_code,
            "phase":       phase,
            "key_signal":  key_signal,
            "tdd":         tdd_evidence,
            "stdout_tail": stdout_stored,
            "stderr_tail": stderr_stored,
        })),
        phase:    params.phase.clone(),
        node_id:  params.node_id.clone(),
        run_id:   params.run_id.clone(),
        ts:       ts.clone(),
        trace_id: None,
    };
    let recorded = journal::append_event(workflow_dir, evt).is_ok();

    // Update verify-staging.json using existing read/write API
    let staging_path = workflow_dir.join("verify-staging.json");
    let mut staging  = verify_staging::read_staging(&staging_path);
    if let Some(phase_key) = verify_staging::classify_phase(&params.command) {
        let entry = serde_json::json!({
            "command":    normalized,
            "exit_code":  exit_code,
            "ts":         ts,
            "node_id":    params.node_id,
            "run_id":     params.run_id,
        });
        staging[phase_key] = entry.clone();
        verify_staging::write_staging(&staging_path, &staging);
        let report_path = workflow_dir.join("ai.report.json");
        verify_staging::sync_to_report(&report_path, phase_key, &entry);
    }

    Ok(PosttoolBashResult {
        kind:         "recorded".into(),
        command:      params.command,
        exit_code,
        phase:        phase.to_string(),
        key_signal:   Some(key_signal),
        tdd_evidence: tdd_evidence.map(str::to_string),
        recorded,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn detect_tdd_evidence(stdout: &str, stderr: &str, exit_code: i32, phase: &str) -> Option<&'static str> {
    if phase != "test" { return None; }
    let combined = format!("{} {}", stdout, stderr).to_lowercase();
    if exit_code == 0 && (combined.contains("test result: ok") || combined.contains("passed")) {
        Some("green")
    } else if exit_code != 0 && (combined.contains("failed") || combined.contains("error")) {
        Some("red")
    } else {
        None
    }
}

fn truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Take tail (most relevant part)
    let start = s.len().saturating_sub(max_bytes);
    format!("[...truncated]\n{}", &s[start..])
}
