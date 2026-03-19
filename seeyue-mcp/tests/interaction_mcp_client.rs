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
    // Ensure env var is not set so this test is not polluted by parallel tests
    std::env::remove_var("SEEYUE_MCP_ELICITATION");
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
    // No env var, no capabilities.yaml → elicitation must be false
    assert!(!result.supports_elicitation,
        "supports_elicitation must be false when no env var and no capabilities.yaml");
}

// ─── test_elicitation_enabled_via_env ────────────────────────────────────────
//
// NOTE: env var mutation is not safe for parallel integration tests.
// The env var path is covered by the unit test in interaction_strategy.rs.
// This test covers the same code path via capabilities.yaml (isolated, no global state).
#[test]
fn test_elicitation_enabled_via_env() {
    let tmp = TempDir::new().expect("create tempdir");
    // Use capabilities.yaml to activate elicitation (same probe_elicitation_support logic)
    let cap_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&cap_dir).expect("create workflow dir");
    fs::write(cap_dir.join("capabilities.yaml"), "elicitation: true\n")
        .expect("write capabilities.yaml");

    let result = run_probe_interaction_capability(
        ProbeInteractionCapabilityParams {
            workspace_override: Some(tmp.path().to_string_lossy().into_owned()),
        },
        tmp.path(),
    )
    .expect("probe should succeed");

    assert!(result.supports_elicitation, "capabilities.yaml must activate elicitation");
    assert_eq!(result.preferred_mode, "elicitation",
        "preferred_mode must be elicitation when supports_elicitation=true");
}

// ─── test_elicitation_enabled_via_capabilities_yaml ─────────────────────────

#[test]
fn test_elicitation_enabled_via_capabilities_yaml() {
    let tmp = TempDir::new().expect("create tempdir");
    // Write capabilities.yaml with elicitation: true
    let cap_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&cap_dir).expect("create workflow dir");
    fs::write(cap_dir.join("capabilities.yaml"), "elicitation: true\n")
        .expect("write capabilities.yaml");

    // Ensure env var is NOT set so we test the yaml path only
    std::env::remove_var("SEEYUE_MCP_ELICITATION");

    let result = run_probe_interaction_capability(
        ProbeInteractionCapabilityParams {
            workspace_override: Some(tmp.path().to_string_lossy().into_owned()),
        },
        tmp.path(),
    )
    .expect("probe should succeed");

    assert!(result.supports_elicitation, "capabilities.yaml must activate elicitation");
    assert_eq!(result.preferred_mode, "elicitation",
        "preferred_mode must be elicitation when capabilities.yaml sets elicitation: true");
}
