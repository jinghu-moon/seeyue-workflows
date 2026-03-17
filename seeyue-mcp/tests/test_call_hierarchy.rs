// tests/test_call_hierarchy.rs
//
// Tests for tools::call_hierarchy::run_call_hierarchy.
// Run: cargo test --test test_call_hierarchy

use seeyue_mcp::tools::call_hierarchy::{CallHierarchyParams, run_call_hierarchy};

fn ws() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn params(symbol: &str, direction: &str) -> CallHierarchyParams {
    CallHierarchyParams {
        symbol:    symbol.into(),
        direction: Some(direction.into()),
        limit:     Some(20),
        path:      None,
    }
}

#[test]
fn test_call_hierarchy_returns_success() {
    let result = run_call_hierarchy(params("run_read", "callers"), &ws()).unwrap();
    assert_eq!(result.kind, "success");
}

#[test]
fn test_call_hierarchy_symbol_matches() {
    let result = run_call_hierarchy(params("run_edit", "both"), &ws()).unwrap();
    assert_eq!(result.symbol, "run_edit");
}

#[test]
fn test_call_hierarchy_callers_direction() {
    let result = run_call_hierarchy(params("run_read", "callers"), &ws()).unwrap();
    assert_eq!(result.direction, "callers");
    // All sites must be callers
    for site in &result.sites {
        assert_eq!(site.kind, "caller");
    }
}

#[test]
fn test_call_hierarchy_callees_finds_definitions() {
    // "run_git_status" is defined somewhere in src/
    let result = run_call_hierarchy(params("run_git_status", "callees"), &ws()).unwrap();
    assert_eq!(result.kind, "success");
    // Should find at least the definition
    assert!(result.total >= 1);
}

#[test]
fn test_call_hierarchy_empty_symbol_errors() {
    let err = run_call_hierarchy(
        CallHierarchyParams { symbol: "".into(), direction: None, limit: None, path: None },
        &ws(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("MissingParameter") || msg.contains("symbol"), "unexpected: {msg}");
}

#[test]
fn test_call_hierarchy_path_escape_blocked() {
    let err = run_call_hierarchy(
        CallHierarchyParams {
            symbol:    "foo".into(),
            direction: None,
            limit:     None,
            path:      Some("../../outside".into()),
        },
        &ws(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("PathEscape") || msg.contains("outside"), "unexpected: {msg}");
}

#[test]
fn test_call_hierarchy_total_matches_sites_len_when_not_truncated() {
    let result = run_call_hierarchy(params("run_read", "both"), &ws()).unwrap();
    if !result.truncated {
        assert_eq!(result.total, result.sites.len());
    }
}
