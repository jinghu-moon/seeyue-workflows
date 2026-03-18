//! Integration tests: terminal capability probing.

use seeyue_mcp::interaction::terminal::{ColorDepth, probe_terminal};

/// probe_terminal() returns TerminalCapabilities with valid fields
#[test]
fn test_probe_returns_struct() {
    let caps = probe_terminal();
    // Basic invariants regardless of environment
    assert!(caps.columns > 0, "columns must be > 0");
    assert!(caps.rows > 0, "rows must be > 0");
    // In a non-TTY test environment:
    if !caps.is_tty {
        assert!(!caps.ansi_enabled, "ansi_enabled must be false when not a TTY");
        assert!(!caps.supports_raw_mode, "supports_raw_mode must be false when not a TTY");
        assert!(!caps.supports_alternate_screen);
        assert_eq!(caps.preferred_mode, "plain");
    }
    // preferred_mode must be one of the known values
    assert!(
        matches!(caps.preferred_mode.as_str(), "plain" | "text" | "tui"),
        "unexpected preferred_mode: {}",
        caps.preferred_mode
    );
}

/// ColorDepth serializes to schema-aligned values
#[test]
fn test_color_depth_variants() {
    let variants: &[(ColorDepth, &str)] = &[
        (ColorDepth::Mono, "mono"),
        (ColorDepth::Ansi16, "ansi16"),
        (ColorDepth::Ansi256, "ansi256"),
        (ColorDepth::TrueColor, "true_color"),
    ];
    for (variant, expected_str) in variants {
        let json = serde_json::to_string(variant).unwrap();
        assert_eq!(
            json,
            format!("\"{}\"", expected_str),
            "ColorDepth::{:?} must serialize to '{}'",
            variant,
            expected_str
        );
        let decoded: ColorDepth = serde_json::from_str(&json).unwrap();
        assert_eq!(&decoded, variant, "round-trip must preserve variant");
    }
}

/// probe_terminal JSON round-trips cleanly
#[test]
fn test_probe_terminal_serialization() {
    let caps = probe_terminal();
    let json = serde_json::to_string_pretty(&caps).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    // All required fields must be present
    assert!(v.get("is_tty").is_some());
    assert!(v.get("color_depth").is_some());
    assert!(v.get("columns").is_some());
    assert!(v.get("rows").is_some());
    assert!(v.get("ansi_enabled").is_some());
    assert!(v.get("supports_raw_mode").is_some());
    assert!(v.get("supports_alternate_screen").is_some());
    assert!(v.get("preferred_mode").is_some());
}
