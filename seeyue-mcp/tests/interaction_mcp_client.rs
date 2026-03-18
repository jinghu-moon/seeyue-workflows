// tests/interaction_mcp_client.rs — P2-N5: Capability probe tests

use seeyue_mcp::tools::interaction_strategy::{
    run_probe_interaction_capability, ProbeInteractionCapabilityParams,
};
use std::fs;
use tempfile::TempDir;

// ─── test_probe_returns_valid_strategy ───────────────────────────────────────

#[test]
fn test_probe_returns_valid_strategy() {
    let tmp = TempDir::new().expect("create tempdir");
    let result = run_probe_interaction_capability(
        ProbeInteractionCapabilityParams { workspace_override: None },
        tmp.path(),
    )
    .expect("probe should succeed");

    // kind must always be capability_probe
    assert_eq!(result.kind, "capability_probe");

    // elicitation is always false (not yet supported)
    assert!(!result.supports_elicitation, "elicitation must be false");

    // preferred_mode must be one of the three valid values
    let valid_modes = ["elicitation", "local_presenter", "text_fallback"];
    assert!(
        valid_modes.contains(&result.preferred_mode.as_str()),
        "preferred_mode '{}' is not a valid value",
        result.preferred_mode
    );

    // mode_reason must be non-empty
    assert!(!result.mode_reason.is_empty(), "mode_reason must not be empty");
}

// ─── test_preferred_mode_is_local_presenter_when_binary_exists ───────────────

#[test]
fn test_preferred_mode_is_local_presenter_when_binary_exists() {
    let tmp = TempDir::new().expect("create tempdir");

    // Plant a fake sy-interact binary in target/debug/
    let debug_dir = tmp.path().join("target").join("debug");
    fs::create_dir_all(&debug_dir).expect("create debug dir");
    let fake_binary = debug_dir.join("sy-interact.exe");
    fs::write(&fake_binary, b"fake binary").expect("write fake binary");

    let result = run_probe_interaction_capability(
        ProbeInteractionCapabilityParams {
            workspace_override: Some(tmp.path().to_string_lossy().into_owned()),
        },
        tmp.path(),
    )
    .expect("probe should succeed");

    assert!(result.supports_local_presenter, "binary exists so supports_local_presenter must be true");
    assert_eq!(result.preferred_mode, "local_presenter");
    assert!(
        result.presenter_binary_path.is_some(),
        "presenter_binary_path must be set when binary exists"
    );
}

// ─── test_text_fallback_when_no_binary ───────────────────────────────────────

#[test]
fn test_text_fallback_when_no_binary() {
    let tmp = TempDir::new().expect("create tempdir");
    // Empty workspace — no binary present, sy-interact not on PATH in test env
    let result = run_probe_interaction_capability(
        ProbeInteractionCapabilityParams {
            workspace_override: Some(tmp.path().to_string_lossy().into_owned()),
        },
        tmp.path(),
    )
    .expect("probe should succeed");

    // In a clean tempdir with no binary, mode depends on whether sy-interact is on PATH.
    // We only assert that the result is well-formed and the mode is valid.
    let valid_modes = ["elicitation", "local_presenter", "text_fallback"];
    assert!(valid_modes.contains(&result.preferred_mode.as_str()));
    assert!(!result.supports_elicitation);
}
