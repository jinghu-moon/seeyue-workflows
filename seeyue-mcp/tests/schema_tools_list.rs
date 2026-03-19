// tests/schema_tools_list.rs
// TDD tests for M-N2: tools/list generated from registry().
// Run: cargo test --test schema_tools_list

use seeyue_mcp::server::schema::generate_tools_list;
use seeyue_mcp::tools::metadata::registry;

// M-N2 test 1: generate_tools_list includes all registry tool names
#[test]
fn test_tools_list_includes_all_registry_tools() {
    let list = generate_tools_list();
    let reg = registry();
    for name in reg.keys() {
        assert!(
            list.iter().any(|t| t.name == *name),
            "tools/list missing tool: {}", name
        );
    }
}

// M-N2 test 2: read_file has read_only_hint=true
#[test]
fn test_read_file_annotations() {
    let list = generate_tools_list();
    let tool = list.iter().find(|t| t.name == "read_file");
    assert!(tool.is_some(), "read_file should be in tools list");
    let t = tool.unwrap();
    assert!(t.read_only_hint, "read_file should have read_only_hint=true");
    assert!(!t.destructive_hint, "read_file should have destructive_hint=false");
}

// M-N2 test 3: memory_delete has destructive_hint=true
#[test]
fn test_memory_delete_annotations() {
    let list = generate_tools_list();
    let tool = list.iter().find(|t| t.name == "memory_delete");
    assert!(tool.is_some());
    assert!(tool.unwrap().destructive_hint);
}

// M-N2 test 4: no duplicate tool names
#[test]
fn test_no_duplicate_tool_names() {
    let list = generate_tools_list();
    let mut names: Vec<&str> = list.iter().map(|t| t.name.as_str()).collect();
    names.sort();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "tools/list has duplicate tool names");
}

// M-N2 test 5: all tools have non-empty descriptions
#[test]
fn test_all_tools_have_descriptions() {
    let list = generate_tools_list();
    for t in &list {
        assert!(!t.description.is_empty(),
            "tool '{}' has empty description", t.name);
    }
}
