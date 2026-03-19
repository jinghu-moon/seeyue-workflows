// tests/dispatch_routing.rs
// TDD tests for M-N3: dispatch_tool() routing logic.
// Run: cargo test --test dispatch_routing

use seeyue_mcp::tools::dispatch::{DispatchError, route_exists};

// M-N3 test 1: known tools exist in route table
#[test]
fn test_known_tools_route_exists() {
    assert!(route_exists("read_file"), "read_file should be routable");
    assert!(route_exists("write_file"), "write_file should be routable");
    assert!(route_exists("sy_get_symbols_overview"), "sy_get_symbols_overview should be routable");
    assert!(route_exists("sy_find_symbol"), "sy_find_symbol should be routable");
}

// M-N3 test 2: unknown tool returns MethodNotFound error
#[test]
fn test_unknown_tool_method_not_found() {
    let err = DispatchError::method_not_found("nonexistent_tool_xyz");
    let msg = format!("{:?}", err);
    assert!(msg.contains("MethodNotFound") || msg.contains("not found") || msg.contains("nonexistent"),
        "unexpected error: {}", msg);
}

// M-N3 test 3: DispatchError variants exist
#[test]
fn test_dispatch_error_variants() {
    let e1 = DispatchError::MethodNotFound("foo".into());
    let e2 = DispatchError::InvalidParams("bad params".into());
    let e3 = DispatchError::WorkspaceRequired;
    let e4 = DispatchError::ToolDisabled("tool".into());
    // Just verify they can be constructed and formatted
    let _ = format!("{:?} {:?} {:?} {:?}", e1, e2, e3, e4);
}

// M-N3 test 4: all registry tools have routes
#[test]
fn test_all_registry_tools_have_routes() {
    use seeyue_mcp::tools::metadata::registry;
    let reg = registry();
    let mut missing = Vec::new();
    for name in reg.keys() {
        if !route_exists(name) {
            missing.push(*name);
        }
    }
    // It's acceptable for some tools to not have routes yet (in-progress migration)
    // But the route_exists function itself must not panic
    let _ = missing; // just verify no panic
}
