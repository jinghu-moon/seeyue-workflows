// src/hooks/verify_staging.rs
//
// Verification staging helpers: phase classification, signal extraction,
// failure kind classification, and staging/report JSON I/O.
//
// Used by PostToolUse:Bash handler to record verification evidence.

use std::fs;
use std::path::Path;

use regex::Regex;
use serde_json::{json, Value};

// ─── Phase Classifiers ──────────────────────────────────────────────────────

/// Classify a command into a verification phase.
/// Returns None if the command doesn't match any known verification pattern.
pub fn classify_phase(command: &str) -> Option<&'static str> {
    // Build patterns lazily — each call does regex compilation.
    // For a hook binary that runs once and exits, this is acceptable.
    static CLASSIFIERS: &[(&str, &[&str])] = &[
        (
            "build",
            &[
                r"(?i)\bcargo\s+build\b",
                r"(?i)\bnpm\s+run\s+build\b",
                r"(?i)\bvite\s+build\b",
                r"(?i)\bgo\s+build\b",
            ],
        ),
        (
            "typecheck",
            &[
                r"(?i)\bcargo\s+check\b",
                r"(?i)\btsc\s+--no-emit\b",
                r"(?i)\bvue-tsc\b",
                r"(?i)\bgo\s+vet\b",
                r"(?i)\bmypy\b",
            ],
        ),
        (
            "lint",
            &[
                r"(?i)\bcargo\s+clippy\b",
                r"(?i)\beslint\b",
                r"(?i)\bruff\s+check\b",
                r"(?i)\bflake8\b",
                r"(?i)\bgolangci-lint\b",
            ],
        ),
        (
            "test",
            &[
                r"(?i)\bcargo\s+test\b",
                r"(?i)\bnpm\s+test\b",
                r"(?i)\bvitest\s+run\b",
                r"(?i)\bpytest\b",
                r"(?i)\bgo\s+test\b",
                r"(?i)\bjest\b",
                r"(?i)\bmocha\b",
            ],
        ),
        (
            "security",
            &[
                r"(?i)\bcargo\s+audit\b",
                r"(?i)\bnpm\s+audit\b",
                r"(?i)\btrufflehog\b",
                r"(?i)\bgitleaks\b",
                r"(?i)\bsemgrep\b",
            ],
        ),
    ];

    for (phase, patterns) in CLASSIFIERS {
        for pat in *patterns {
            if let Ok(re) = Regex::new(pat) {
                if re.is_match(command) {
                    return Some(phase);
                }
            }
        }
    }

    None
}

// ─── Signal Extraction ──────────────────────────────────────────────────────

/// Extract a key signal from command output for recording.
pub fn extract_key_signal(stdout: &str, stderr: &str, exit_code: i64) -> String {
    let combined = format!("{}\n{}", stdout, stderr);
    let lines: Vec<&str> = combined.lines().collect();

    // Sample: first 5 + last 20 lines
    let sample = {
        let head: Vec<&str> = lines.iter().take(5).copied().collect();
        let tail_start = if lines.len() > 20 { lines.len() - 20 } else { 0 };
        let tail: Vec<&str> = lines[tail_start..].to_vec();
        let mut s = head;
        s.extend(tail);
        s.join("\n")
    };

    if exit_code == 0 {
        let pass_signals: &[&str] = &[
            r"(?i)\b0\s+(errors?|failed|failures?)\b",
            r"(?i)\bfinished\b.*\b0\s+errors?\b",
            r"(?i)\ball\s+tests?\s+pass(ed)?\b",
            r"(?i)\btest\s+result\b.*\bok\b",
            r"(?i)\bBUILD\s+SUCCESSFUL\b",
            r"(?i)\bbuilt\s+in\b",
            r"(?i)\bno\s+(issues?|errors?|warnings?)\s+found\b",
        ];

        for pat in pass_signals {
            if let Ok(re) = Regex::new(pat) {
                if let Some(m) = re.find(&sample) {
                    let sig = m.as_str().trim();
                    return truncate(sig, 100).to_string();
                }
            }
        }
        return "exit 0".to_string();
    }

    let fail_signals: &[&str] = &[
        r"(?i)\b\d+\s+(errors?|failures?|failed)\b",
        r"(?i)\bBUILD\s+FAILED\b",
        r"(?i)\berror\[E\d+\]",
        r"\bFAILED\b",
        r"(?i)\btest\s+result\b.*\bFAILED\b",
    ];

    for pat in fail_signals {
        if let Ok(re) = Regex::new(pat) {
            if let Some(m) = re.find(&sample) {
                let sig = m.as_str().trim();
                return truncate(sig, 100).to_string();
            }
        }
    }

    // Fallback: find an error/FAIL line
    if let Some(err_line) = lines
        .iter()
        .find(|line| {
            let lower = line.to_lowercase();
            (lower.contains("error") || lower.contains("fail")) && !line.trim().is_empty()
        })
    {
        return truncate(err_line.trim(), 100).to_string();
    }

    format!("exit {}", exit_code)
}

// ─── Failure Kind Classification ────────────────────────────────────────────

/// Classify the kind of failure from command output.
pub fn classify_failure_kind(stdout: &str, stderr: &str, exit_code: i64) -> String {
    if exit_code == 0 {
        return "unexpected_pass".to_string();
    }

    let combined = format!("{}\n{}", stdout.to_lowercase(), stderr.to_lowercase());

    static PATTERNS: &[(&str, &[&str])] = &[
        ("syntax_error", &["syntaxerror", "unexpected token", "parse error"]),
        ("import_error", &["module not found", "cannot find module", "importerror", "no module named"]),
        ("permission_error", &["permission denied", "eacces", "eperm"]),
        ("connection_error", &["econnrefused", "enotfound", "timed out", "timeout", "connection refused"]),
        ("fixture_initialization_error", &["fixture", "beforeall", "before each", "setup failed"]),
        ("contract_mismatch", &["contract", "schema mismatch"]),
        ("expected_validation_failure", &["validation failed", "expected validation"]),
        ("behavior_result_mismatch", &["behavior mismatch", "unexpected behavior"]),
        ("assertion_failure", &["assertion", "expected .* to", "should", "expect", "assert"]),
    ];

    for (kind, needles) in PATTERNS {
        for needle in *needles {
            if combined.contains(needle) {
                return kind.to_string();
            }
        }
    }

    "environment_error".to_string()
}

// ─── Staging JSON I/O ───────────────────────────────────────────────────────

/// Read verify-staging.json. Returns empty object on missing/invalid file.
pub fn read_staging(staging_path: &Path) -> Value {
    match fs::read_to_string(staging_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    }
}

/// Write verify-staging.json. Creates parent directories as needed.
/// Non-fatal on error.
pub fn write_staging(staging_path: &Path, data: &Value) {
    if let Some(parent) = staging_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(data) {
        let _ = fs::write(staging_path, content);
    }
}

/// Sync a verification phase entry to ai.report.json.
/// Updates the `verification` section and recalculates `overall` status.
pub fn sync_to_report(report_path: &Path, phase: &str, phase_entry: &Value) {
    let content = match fs::read_to_string(report_path) {
        Ok(c) => c,
        Err(_) => return, // Report doesn't exist — skip
    };

    let mut report: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Ensure report.verification exists
    if report.get("verification").is_none() {
        return;
    }

    let status = phase_entry
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let update_obj = json!({
        "status": status,
        "command": phase_entry.get("command"),
        "exit_code": phase_entry.get("exit_code"),
        "key_signal": phase_entry.get("key_signal"),
        "source": "verify-staging",
        "recorded_at": phase_entry.get("ts"),
    });

    let verif = report.get_mut("verification").unwrap();

    match phase {
        "build" => {
            verif["build"] = update_obj;
        }
        "typecheck" => {
            verif["typecheck"] = update_obj;
            verif["compile"] = Value::String(status.to_string());
        }
        "lint" => {
            verif["lint"] = update_obj;
        }
        "test" => {
            verif["tests"] = json!([{
                "status": status,
                "command": phase_entry.get("command"),
                "exit_code": phase_entry.get("exit_code"),
                "key_signal": phase_entry.get("key_signal"),
                "source": "verify-staging",
                "recorded_at": phase_entry.get("ts"),
            }]);
            verif["test"] = Value::String(status.to_string());
        }
        "security" => {
            verif["security"] = update_obj;
        }
        _ => {}
    }

    // Update timestamp
    report["updated_at"] = Value::String(chrono::Utc::now().to_rfc3339());

    // Recalculate overall status
    let has_fail = {
        let v = report.get("verification").unwrap();
        let checks = [
            v.get("build").and_then(|b| b.get("status")).and_then(|s| s.as_str()),
            v.get("typecheck").and_then(|b| b.get("status")).and_then(|s| s.as_str()),
            v.get("lint").and_then(|b| b.get("status")).and_then(|s| s.as_str()),
            v.get("test").and_then(|s| s.as_str()),
            v.get("security").and_then(|b| b.get("status")).and_then(|s| s.as_str()),
            v.get("compile").and_then(|s| s.as_str()),
        ];
        checks.iter().any(|s| *s == Some("fail"))
    };

    if has_fail {
        report["overall"] = Value::String("NOT_READY".to_string());
    }

    // Write back
    if let Ok(out) = serde_json::to_string_pretty(&report) {
        let _ = fs::write(report_path, out);
    }
}

/// Normalize command: trim + collapse whitespace.
pub fn normalize_command(command: &str) -> String {
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(command.trim(), " ").to_string()
}

/// Check if a command matches an expected command (exact or substring).
pub fn command_matches(normalized: &str, expected: &str) -> bool {
    let norm_expected = normalize_command(expected);
    if norm_expected.is_empty() {
        return false;
    }
    normalized == norm_expected || normalized.contains(&norm_expected)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
