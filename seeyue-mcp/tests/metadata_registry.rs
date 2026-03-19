// tests/metadata_registry.rs
//
// TDD tests for ToolMetadata registry (M-N1).
// Run: cargo test --test metadata_registry

use seeyue_mcp::tools::metadata::{registry, ToolMetadata, ToolCategory};

// M-N1 test 1: registry() returns non-empty map
#[test]
fn test_registry_non_empty() {
    let reg = registry();
    assert!(!reg.is_empty(), "registry should contain at least one tool");
}

// M-N1 test 2: all tool names are unique (enforced by HashMap keys)
#[test]
fn test_all_tool_names_unique() {
    let reg = registry();
    // HashMap keys are inherently unique — just verify count is sane
    assert!(reg.len() >= 5, "registry should have at least 5 tools, got {}", reg.len());
}

// M-N1 test 3: get() returns correct metadata for a known tool
#[test]
fn test_get_known_tool() {
    let meta = ToolMetadata::get("read_file");
    assert!(meta.is_some(), "read_file should be in registry");
    let m = meta.unwrap();
    assert_eq!(m.name, "read_file");
    assert!(m.read_only, "read_file should be read_only");
    assert!(!m.destructive, "read_file should not be destructive");
    assert!(m.active_by_default, "read_file should be active by default");
}

// M-N1 test 4: is_active with empty active_tools set respects active_by_default
#[test]
fn test_is_active_respects_active_by_default() {
    use std::collections::HashSet;
    let empty: HashSet<String> = HashSet::new();

    // read_file is active_by_default=true
    assert!(ToolMetadata::is_active("read_file", &empty),
        "read_file should be active when active_by_default=true");
}

// M-N1 test 5: is_active with non-default tool enabled via active_tools set
#[test]
fn test_is_active_via_active_tools_set() {
    use std::collections::HashSet;
    let mut active: HashSet<String> = HashSet::new();
    // Add a tool that is active_by_default=false
    active.insert("sy_rename_symbol".to_string());
    // It should be active because it's in the set
    assert!(ToolMetadata::is_active("sy_rename_symbol", &active),
        "sy_rename_symbol should be active when in active_tools set");
}

// M-N1 test 6: get() returns None for unknown tool
#[test]
fn test_get_unknown_tool_returns_none() {
    let meta = ToolMetadata::get("nonexistent_xyz_tool");
    assert!(meta.is_none());
}

// M-N1 test 7: ToolCategory variants exist and are comparable
#[test]
fn test_tool_category_equality() {
    assert_eq!(ToolCategory::FileEdit, ToolCategory::FileEdit);
    assert_ne!(ToolCategory::FileEdit, ToolCategory::Git);
    assert_ne!(ToolCategory::Nav, ToolCategory::Symbol);
}
