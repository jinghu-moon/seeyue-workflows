// tests/test_edit.rs
//
// Functional tests for tools::edit::apply_edit_in_memory.
// Pure in-memory tests — no filesystem, no checkpoint, no workspace.
// Run: cargo test --test test_edit

use seeyue_mcp::tools::edit::apply_edit_in_memory;

// ─── Basic replace ────────────────────────────────────────────────────────────

#[test]
fn test_simple_replace() {
    let content = "hello world";
    let result = apply_edit_in_memory("world", "rust", false, content).unwrap();
    assert_eq!(result.new_content, "hello rust");
    assert_eq!(result.replacements, 1);
}

#[test]
fn test_multiple_matches_without_replace_all_errors() {
    // apply_edit_in_memory enforces uniqueness: multiple matches → MultipleMatches error
    let content = "aaa bbb aaa";
    let result = apply_edit_in_memory("aaa", "XXX", false, content);
    assert!(result.is_err(), "multiple matches with replace_all=false should error");
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("MultipleMatches") || msg.contains("multiple") || msg.contains("2"),
        "error should mention multiple matches: {msg}");
}

#[test]
fn test_replace_all_occurrences() {
    let content = "foo bar foo baz foo";
    let result = apply_edit_in_memory("foo", "qux", true, content).unwrap();
    assert_eq!(result.new_content, "qux bar qux baz qux");
    assert_eq!(result.replacements, 3);
}

#[test]
fn test_replace_multiline() {
    let content = "line1\nline2\nline3\n";
    let result = apply_edit_in_memory("line2\n", "replaced\n", false, content).unwrap();
    assert_eq!(result.new_content, "line1\nreplaced\nline3\n");
}

#[test]
fn test_insert_at_beginning() {
    let content = "existing content";
    let result = apply_edit_in_memory("existing", "new existing", false, content).unwrap();
    assert_eq!(result.new_content, "new existing content");
}

#[test]
fn test_delete_by_replacing_with_empty() {
    let content = "keep this\ndelete this line\nkeep that";
    let result = apply_edit_in_memory("\ndelete this line", "", false, content).unwrap();
    assert_eq!(result.new_content, "keep this\nkeep that");
}

// ─── Error cases ──────────────────────────────────────────────────────────────

#[test]
fn test_old_string_not_found_returns_error() {
    let content = "hello world";
    let result = apply_edit_in_memory("nonexistent", "new", false, content);
    assert!(result.is_err(), "missing old_string should return error");
}

#[test]
fn test_empty_content_not_found() {
    let result = apply_edit_in_memory("something", "else", false, "");
    assert!(result.is_err(), "searching in empty content should error");
}

// ─── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn test_replace_with_same_text_is_noop() {
    // old == new: content unchanged, replacements = 1
    let content = "abc";
    let result = apply_edit_in_memory("abc", "abc", false, content).unwrap();
    assert_eq!(result.new_content, "abc");
}

#[test]
fn test_unicode_replace() {
    let content = "你好世界";
    let result = apply_edit_in_memory("世界", "Rust", false, content).unwrap();
    assert_eq!(result.new_content, "你好Rust");
}

#[test]
fn test_replace_all_with_single_occurrence() {
    let content = "once";
    let result = apply_edit_in_memory("once", "twice", true, content).unwrap();
    assert_eq!(result.new_content, "twice");
    assert_eq!(result.replacements, 1);
}

#[test]
fn test_whitespace_only_old_string() {
    let content = "line1\n   \nline3";
    let result = apply_edit_in_memory("   ", "", false, content).unwrap();
    assert_eq!(result.new_content, "line1\n\nline3");
}

#[test]
fn test_large_content_replace_all() {
    // With replace_all=true, all 100 occurrences are replaced
    let line = "x".repeat(100) + "\n";
    let content = line.repeat(100);
    let result = apply_edit_in_memory(&"x".repeat(100), &"y".repeat(100), true, &content).unwrap();
    assert!(result.new_content.contains(&"y".repeat(100)));
    assert_eq!(result.replacements, 100);
    assert!(!result.new_content.contains(&"x".repeat(100)));
}
