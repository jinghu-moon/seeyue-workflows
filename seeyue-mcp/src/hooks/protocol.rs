// src/hooks/protocol.rs
//
// Hook protocol: stdin JSON parsing and stdout JSON formatting.
// Implements the Claude Code hook contract:
//   stdin  → HookInput (one-shot JSON read)
//   stdout → HookOutput (single JSON object)
//   exit   → 0 (allow/force_continue) | 2 (block/block_with_approval_request)

use std::collections::HashMap;
use std::io::Read;
use std::process;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::policy::types::{HookResult, Verdict};

// ─── Input ──────────────────────────────────────────────────────────────────

/// Deserialized stdin JSON from Claude Code hook protocol.
#[derive(Debug, Deserialize, Default)]
pub struct HookInput {
    /// Hook event identifier (e.g. "PreToolUse:Bash", "SessionStart").
    /// Not always present — thin-shell hooks rely on argv/dispatch, but
    /// the Rust binary infers it from the matcher context.
    #[serde(default)]
    pub hook_event: Option<String>,

    /// Tool name for PreToolUse / PostToolUse events.
    #[serde(default)]
    pub tool_name: Option<String>,

    /// Tool input payload (command, file_path, etc.).
    #[serde(default)]
    pub tool_input: Option<Value>,

    /// Tool response payload (stdout, stderr, exit_code — PostToolUse only).
    #[serde(default)]
    pub tool_response: Option<Value>,

    /// Working directory of the Claude Code session.
    #[serde(default)]
    pub cwd: Option<String>,

    /// Session identifier.
    #[serde(default)]
    pub session_id: Option<String>,

    /// User prompt text (UserPromptSubmit only).
    #[serde(default)]
    pub prompt: Option<String>,

    /// Alternative prompt field name used by some hook versions.
    #[serde(default)]
    pub message: Option<String>,

    /// Catch-all for unknown fields — forward compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl HookInput {
    /// Read and parse stdin as a single JSON object.
    /// Returns `HookInput::default()` on empty stdin or parse failure (fail-open).
    pub fn from_stdin() -> Self {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
            return Self::default();
        }
        serde_json::from_str(&buf).unwrap_or_default()
    }

    /// Resolve the effective CWD: payload field → env var → process cwd.
    pub fn resolve_cwd(&self) -> String {
        if let Some(cwd) = &self.cwd {
            if !cwd.is_empty() {
                return cwd.clone();
            }
        }
        if let Ok(env_dir) = std::env::var("CLAUDE_PROJECT_DIR") {
            if !env_dir.is_empty() {
                return env_dir;
            }
        }
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    }

    /// Extract a string field from `tool_input`.
    pub fn tool_input_str(&self, key: &str) -> Option<String> {
        self.tool_input
            .as_ref()?
            .get(key)?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Extract a string field from `tool_response`.
    pub fn tool_response_str(&self, key: &str) -> Option<String> {
        self.tool_response
            .as_ref()?
            .get(key)?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Extract an integer field from `tool_response`, trying multiple field names.
    pub fn tool_response_int(&self, keys: &[&str]) -> Option<i64> {
        let resp = self.tool_response.as_ref()?;
        for key in keys {
            if let Some(v) = resp.get(*key) {
                if let Some(n) = v.as_i64() {
                    return Some(n);
                }
                // Also try parsing string representation
                if let Some(s) = v.as_str() {
                    if let Ok(n) = s.parse::<i64>() {
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    /// Resolve user prompt text from either `prompt` or `message` field.
    pub fn resolve_prompt(&self) -> String {
        self.prompt
            .as_deref()
            .or(self.message.as_deref())
            .unwrap_or("")
            .to_string()
    }
}

// ─── Output ─────────────────────────────────────────────────────────────────

/// Output envelope written to stdout.
#[derive(Debug, Serialize)]
pub struct HookOutput {
    pub verdict: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub instructions: Vec<String>,
    /// Extra fields merged into the top-level output (e.g. additional_context).
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Emit a HookResult to stdout and exit with the appropriate code.
///
/// Extra fields (e.g. `additional_context` for SessionStart) can be provided
/// via the `extra` parameter and will be merged into the output envelope.
pub fn emit_result(result: HookResult, extra: Option<HashMap<String, Value>>) -> ! {
    let exit_code = match result.verdict {
        Verdict::Block | Verdict::BlockWithApprovalRequest => 2,
        Verdict::Allow | Verdict::ForceContinue => 0,
    };

    let output = HookOutput {
        verdict: result.verdict.to_string(),
        reason: result.reason,
        instructions: result.instructions,
        extra: extra.unwrap_or_default(),
    };

    // Write JSON to stdout — ignore errors (nothing we can do).
    let _ = serde_json::to_writer(std::io::stdout().lock(), &output);
    process::exit(exit_code);
}

/// Convenience: emit an allow result and exit(0).
pub fn emit_allow(reason: &str) -> ! {
    emit_result(HookResult::allow(reason), None)
}

/// Convenience: emit an allow result with extra fields and exit(0).
pub fn emit_allow_with_extra(reason: &str, extra: HashMap<String, Value>) -> ! {
    emit_result(HookResult::allow(reason), Some(extra))
}
