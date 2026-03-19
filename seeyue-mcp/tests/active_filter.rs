// tests/active_filter.rs
// TDD tests for M-N5: active_tools HashSet + Active Filter.
// Run: cargo test --test active_filter

use std::collections::HashSet;
use seeyue_mcp::tools::metadata::ToolMetadata;
use seeyue_mcp::tools::active_filter::{ActiveFilter, FilterResult};

// M-N5 test 1: active_by_default=true 工具无需 active_tools 即可调用
#[test]
fn test_active_by_default_passes_without_set() {
    let filter = ActiveFilter::new(HashSet::new());
    // "read_file" is active_by_default=true in metadata registry
    let result = filter.check("read_file");
    assert_eq!(result, FilterResult::Allowed,
        "active_by_default tool should be allowed with empty active_tools");
}

// M-N5 test 2: active_by_default=false 工具被调用 → ToolDisabled
#[test]
fn test_not_active_by_default_blocked() {
    let filter = ActiveFilter::new(HashSet::new());
    // "sy_rename_symbol" is active_by_default=false
    let result = filter.check("sy_rename_symbol");
    assert_eq!(result, FilterResult::Disabled,
        "non-default tool should be Disabled with empty active_tools");
}

// M-N5 test 3: active_tools 含该工具 → 可调用
#[test]
fn test_explicit_active_tools_allows_tool() {
    let mut set = HashSet::new();
    set.insert("sy_rename_symbol".to_string());
    let filter = ActiveFilter::new(set);
    let result = filter.check("sy_rename_symbol");
    assert_eq!(result, FilterResult::Allowed,
        "tool explicitly in active_tools should be Allowed");
}

// M-N5 test 4: HashSet 去重（重复项无副作用）
#[test]
fn test_hashset_dedup_no_side_effects() {
    let mut set = HashSet::new();
    set.insert("sy_rename_symbol".to_string());
    set.insert("sy_rename_symbol".to_string()); // duplicate
    let filter = ActiveFilter::new(set);
    // Should not panic or return wrong result
    assert_eq!(filter.check("sy_rename_symbol"), FilterResult::Allowed);
}

// M-N5 test 5: 未知工具名 → Disabled（不在 registry 也不在 active_tools）
#[test]
fn test_unknown_tool_is_disabled() {
    let filter = ActiveFilter::new(HashSet::new());
    let result = filter.check("nonexistent_tool_xyz");
    assert_eq!(result, FilterResult::Disabled,
        "unknown tool should be Disabled");
}

// M-N5 test 6: .ai/workflow/capabilities.yaml 不存在时 active_tools 为空集
#[test]
fn test_load_from_missing_file_yields_empty() {
    let set = seeyue_mcp::tools::active_filter::load_active_tools_from_yaml(
        "/nonexistent/path/capabilities.yaml"
    );
    assert!(set.is_empty(),
        "missing yaml should yield empty active_tools set");
}

// M-N5 test 7: active_tools 读锁不跨 await（同步 API 验证）
#[test]
fn test_filter_check_is_sync() {
    // ActiveFilter::check must be a sync fn (not async)
    // This test compiles only if check() is sync
    let filter = ActiveFilter::new(HashSet::new());
    let _result: FilterResult = filter.check("read");
    // No runtime needed — proves the fn is synchronous
}
