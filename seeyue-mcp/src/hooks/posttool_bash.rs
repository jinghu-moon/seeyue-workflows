// src/hooks/posttool_bash.rs
//
// PostToolUse:Bash handler: verification evidence capture and TDD red/green recording.
//
// This is the most complex new handler. It:
// 1. Extracts command + exit code + stdout/stderr from the tool response
// 2. Captures TDD red/green evidence (matches test_contract.red_cmd / green_cmd)
// 3. Classifies the command into a verification phase (build/typecheck/lint/test/security)
// 4. Updates verify-staging.json and syncs to ai.report.json
// 5. Records journal events for audit trail

use std::path::Path;

use serde_json::json;

use crate::hooks::protocol::{HookInput, emit_allow};
use crate::hooks::verify_staging;
use crate::workflow::journal::{self, JournalEvent};
use crate::workflow::state::SessionState;

/// Handle the PostToolUse:Bash hook event.
pub fn handle(input: &HookInput, workflow_dir: &Path, session: &SessionState) -> ! {
    // 1. Bypass check
    if std::env::var("SY_BYPASS_VERIFY_CAPTURE").unwrap_or_default() == "1" {
        emit_allow("bypass_verify_capture");
    }

    // 2. Extract command
    let command = input
        .tool_input_str("command")
        .or_else(|| input.tool_input_str("cmd"))
        .unwrap_or_default()
        .trim()
        .to_string();

    if command.is_empty() {
        emit_allow("empty_command");
    }

    // 3. Extract response fields
    let exit_code = input
        .tool_response_int(&["returncode", "exit_code", "exitCode"])
        .unwrap_or(-1);
    let stdout = input.tool_response_str("stdout").unwrap_or_default();
    let stderr = input.tool_response_str("stderr").unwrap_or_default();

    // 4. Compute signals
    let key_signal = verify_staging::extract_key_signal(&stdout, &stderr, exit_code);
    let normalized = verify_staging::normalize_command(&command);
    let phase = verify_staging::classify_phase(&command);

    // 5. Session context
    let cwd = input.resolve_cwd();
    let run_id = session.run_id.as_deref().unwrap_or("").to_string();
    let node_id = session
        .node
        .id
        .as_deref()
        .or(session.node.name.as_deref())
        .unwrap_or("")
        .to_string();
    let phase_id = session
        .phase
        .id
        .as_deref()
        .or(session.phase.name.as_deref())
        .unwrap_or("none")
        .to_string();

    // 6. TDD red/green evidence capture
    let test_contract = &session.node.test_contract;
    let red_cmd = test_contract
        .as_ref()
        .and_then(|tc| tc.get("red_cmd"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let green_cmd = test_contract
        .as_ref()
        .and_then(|tc| tc.get("green_cmd"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let red_matched = if !red_cmd.is_empty() {
        verify_staging::command_matches(&normalized, &red_cmd)
    } else {
        false
    };
    let green_matched = if !green_cmd.is_empty() {
        verify_staging::command_matches(&normalized, &green_cmd)
    } else {
        false
    };

    // 6a. Record red event
    if !run_id.is_empty() && red_matched {
        let evt = JournalEvent::new("red_recorded", "hook")
            .with_run_id(&run_id)
            .with_phase(&phase_id)
            .with_node_id(&node_id)
            .with_payload(json!({
                "executed": true,
                "testFailed": exit_code != 0,
                "failureKind": verify_staging::classify_failure_kind(&stdout, &stderr, exit_code),
                "exitCode": exit_code,
                "recorded": true,
                "command": command,
                "key_signal": key_signal,
            }));
        let _ = journal::append_event(workflow_dir, evt);
    }

    // 6b. Record green event
    if !run_id.is_empty() && green_matched {
        let evt = JournalEvent::new("green_recorded", "hook")
            .with_run_id(&run_id)
            .with_phase(&phase_id)
            .with_node_id(&node_id)
            .with_payload(json!({
                "executed": true,
                "passed": exit_code == 0,
                "newBlockerIntroduced": exit_code != 0,
                "exitCode": exit_code,
                "recorded": true,
                "command": command,
                "key_signal": key_signal,
            }));
        let _ = journal::append_event(workflow_dir, evt);
    }

    // 7. If not a verification command, done
    let phase = match phase {
        Some(p) => p,
        None => emit_allow("non_verification_command"),
    };

    // 8. Build phase entry
    let ts = chrono::Utc::now().to_rfc3339();
    let phase_entry = json!({
        "command": command,
        "exit_code": exit_code,
        "status": if exit_code == 0 { "pass" } else { "fail" },
        "key_signal": key_signal,
        "ts": ts,
        "node": if node_id.is_empty() { None } else { Some(&node_id) },
    });

    // 9. Update verify-staging.json
    let staging_path = Path::new(&cwd).join(".ai/analysis/verify-staging.json");
    let report_path = Path::new(&cwd).join(".ai/analysis/ai.report.json");

    let mut staging = verify_staging::read_staging(&staging_path);

    // Ensure phases object exists
    if staging.get("phases").is_none() {
        staging["phases"] = json!({});
    }

    // Reset phases if run_id changed
    if !run_id.is_empty() {
        if let Some(existing_run_id) = staging.get("session_run_id").and_then(|v| v.as_str()) {
            if existing_run_id != run_id {
                staging["phases"] = json!({});
            }
        }
    }

    staging["phases"][phase] = phase_entry.clone();
    staging["updated_at"] = json!(ts);
    if !run_id.is_empty() {
        staging["session_run_id"] = json!(run_id);
    }

    verify_staging::write_staging(&staging_path, &staging);

    // 10. Sync to report
    verify_staging::sync_to_report(&report_path, phase, &phase_entry);

    // 11. Record verification event
    if !run_id.is_empty() {
        let evt = JournalEvent::new("verification_recorded", "hook")
            .with_run_id(&run_id)
            .with_phase(&phase_id)
            .with_node_id(&node_id)
            .with_payload(json!({
                "verification_phase": phase,
                "command": command,
                "exit_code": exit_code,
                "status": if exit_code == 0 { "pass" } else { "fail" },
                "key_signal": key_signal,
                "staging_ref": ".ai/analysis/verify-staging.json",
                "report_synced": report_path.exists(),
            }));
        let _ = journal::append_event(workflow_dir, evt);
    }

    emit_allow("allow_posttool_bash")
}
