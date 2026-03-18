// src/tools/interaction_strategy.rs — P2-N5: Interaction capability probe
//
// sy_probe_interaction_capability:
//   Returns JSON with:
//     supports_elicitation       bool  (always false for now — client-dependent)
//     supports_local_presenter   bool  (true if sy-interact binary exists)
//     preferred_mode             'elicitation'|'local_presenter'|'text_fallback'
//   Priority: elicitation > local_presenter > text_fallback

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct ProbeInteractionCapabilityParams {
    // No required params — probe reads the environment
    /// Override workspace path for locating sy-interact binary (optional, for testing).
    pub workspace_override: Option<String>,
}

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ProbeInteractionCapabilityResult {
    #[serde(rename = "type")]
    pub kind:                      String,
    /// MCP protocol-level elicitation (always false until clients support it).
    pub supports_elicitation:      bool,
    /// Local sy-interact binary found and executable.
    pub supports_local_presenter:  bool,
    /// Preferred interaction mode given available capabilities.
    pub preferred_mode:            String, // 'elicitation'|'local_presenter'|'text_fallback'
    /// Path to sy-interact binary if found, null otherwise.
    pub presenter_binary_path:     Option<String>,
    /// Reasoning for preferred_mode selection.
    pub mode_reason:               String,
}

// ─── Binary search ───────────────────────────────────────────────────────────

/// Search for the sy-interact binary in standard locations.
/// Returns the path if found.
fn find_presenter_binary(workspace: &Path) -> Option<String> {
    // 1. Check workspace-relative target/debug and target/release
    let candidates = [
        workspace.join("sy-interact").join("target").join("debug").join("sy-interact.exe"),
        workspace.join("sy-interact").join("target").join("release").join("sy-interact.exe"),
        workspace.join("target").join("debug").join("sy-interact.exe"),
        workspace.join("target").join("release").join("sy-interact.exe"),
        // Unix variants (no .exe)
        workspace.join("sy-interact").join("target").join("debug").join("sy-interact"),
        workspace.join("sy-interact").join("target").join("release").join("sy-interact"),
        workspace.join("target").join("debug").join("sy-interact"),
        workspace.join("target").join("release").join("sy-interact"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    // 2. PATH lookup via `which`
    if let Ok(path) = which::which("sy-interact") {
        return Some(path.to_string_lossy().into_owned());
    }

    None
}

// ─── run_probe_interaction_capability ────────────────────────────────────────

pub fn run_probe_interaction_capability(
    params: ProbeInteractionCapabilityParams,
    workspace: &Path,
) -> Result<ProbeInteractionCapabilityResult, ToolError> {
    // Resolve actual workspace for binary search
    let search_root = params
        .workspace_override
        .as_deref()
        .map(std::path::Path::new)
        .unwrap_or(workspace);

    // Elicitation: always false (MCP elicitation spec not yet widely supported)
    let supports_elicitation = false;

    // Local presenter: check for sy-interact binary
    let presenter_binary_path = find_presenter_binary(search_root);
    let supports_local_presenter = presenter_binary_path.is_some();

    // Select preferred mode
    let (preferred_mode, mode_reason) = if supports_elicitation {
        (
            "elicitation".to_string(),
            "MCP elicitation protocol supported by client".to_string(),
        )
    } else if supports_local_presenter {
        (
            "local_presenter".to_string(),
            "sy-interact binary found; will use TUI presenter for interactions".to_string(),
        )
    } else {
        (
            "text_fallback".to_string(),
            "No elicitation or presenter available; falling back to plain text prompts".to_string(),
        )
    };

    Ok(ProbeInteractionCapabilityResult {
        kind:                     "capability_probe".to_string(),
        supports_elicitation,
        supports_local_presenter,
        preferred_mode,
        presenter_binary_path,
        mode_reason,
    })
}
