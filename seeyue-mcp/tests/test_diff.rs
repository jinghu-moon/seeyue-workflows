// tests/test_diff.rs
//
// Functional tests for compute_diff: correctness of hunks, line numbers,
// summary counts, and rendered output.
// Run: cargo test --test test_diff

use seeyue_mcp::render::diff::{compute_diff, DiffLineKind};

// ─── identical input ─────────────────────────────────────────────────────────

#[test]
fn test_identical_no_hunks() {
    let text = "line1\nline2\nline3\n";
    let result = compute_diff("f.rs", text, text, None);
    assert!(result.hunks.is_empty(), "identical input should produce no hunks");
    assert_eq!(result.summary.total_removed, 0);
    assert_eq!(result.summary.total_added, 0);
}

// ─── single line change ───────────────────────────────────────────────────────

#[test]
fn test_single_line_replace() {
    let result = compute_diff("f.rs", "old\n", "new\n", None);
    assert_eq!(result.summary.total_removed, 1);
    assert_eq!(result.summary.total_added, 1);
    assert_eq!(result.hunks.len(), 1);
}

#[test]
fn test_add_line() {
    let result = compute_diff("f.rs", "a\nb\n", "a\nX\nb\n", None);
    assert_eq!(result.summary.total_added, 1);
    assert_eq!(result.summary.total_removed, 0);
}

#[test]
fn test_delete_line() {
    let result = compute_diff("f.rs", "a\nb\nc\n", "a\nc\n", None);
    assert_eq!(result.summary.total_removed, 1);
    assert_eq!(result.summary.total_added, 0);
}

// ─── multiple hunks ───────────────────────────────────────────────────────────

#[test]
fn test_multiple_hunks() {
    let old: String = (0..20).map(|i| format!("line{}\n", i)).collect();
    let mut new_lines: Vec<String> = (0..20).map(|i| format!("line{}\n", i)).collect();
    new_lines[0] = "CHANGED\n".to_string();
    new_lines[19] = "CHANGED\n".to_string();
    let new = new_lines.join("");
    let result = compute_diff("src/lib.rs", &old, &new, None);
    assert_eq!(result.summary.total_removed, 2);
    assert_eq!(result.summary.total_added, 2);
    assert!(result.hunks.len() >= 2, "expected multiple hunks");
}

// ─── line number correctness ──────────────────────────────────────────────────

#[test]
fn test_del_line_has_old_number() {
    let result = compute_diff("f.rs", "a\nb\nc\n", "a\nc\n", None);
    let del_lines: Vec<_> = result.hunks.iter()
        .flat_map(|h| h.lines.iter())
        .filter(|l| l.kind == DiffLineKind::Del)
        .collect();
    assert!(!del_lines.is_empty());
    for l in &del_lines {
        assert!(l.line_old.is_some(), "Del line must have line_old");
        assert!(l.line_new.is_none(), "Del line must not have line_new");
    }
}

#[test]
fn test_add_line_has_new_number() {
    let result = compute_diff("f.rs", "a\nc\n", "a\nb\nc\n", None);
    let add_lines: Vec<_> = result.hunks.iter()
        .flat_map(|h| h.lines.iter())
        .filter(|l| l.kind == DiffLineKind::Add)
        .collect();
    assert!(!add_lines.is_empty());
    for l in &add_lines {
        assert!(l.line_new.is_some(), "Add line must have line_new");
        assert!(l.line_old.is_none(), "Add line must not have line_old");
    }
}

// ─── rendered output ──────────────────────────────────────────────────────────

#[test]
fn test_plain_rendered_contains_filename() {
    let result = compute_diff("src/main.rs", "old\n", "new\n", None);
    assert!(result.plain_rendered.contains("src/main.rs"),
        "plain output should contain filename");
}

#[test]
fn test_plain_rendered_contains_diff_markers() {
    let result = compute_diff("f.rs", "old\n", "new\n", None);
    assert!(result.plain_rendered.contains('-') || result.plain_rendered.contains('+'),
        "plain output should contain diff markers");
}

#[test]
fn test_hook_note_in_rendered_output() {
    let result = compute_diff("f.rs", "old\n", "new\n", Some("rustfmt"));
    assert!(result.plain_rendered.contains("rustfmt") || result.ansi_rendered.contains("rustfmt"),
        "hook note should appear in rendered output");
}

// ─── summary counts match hunks ───────────────────────────────────────────────

#[test]
fn test_summary_counts_match_hunks() {
    let result = compute_diff("f.rs", "a\nb\nc\n", "a\nX\nY\nc\n", None);
    let hunk_rem: usize = result.hunks.iter().map(|h| h.removed).sum();
    let hunk_add: usize = result.hunks.iter().map(|h| h.added).sum();
    assert_eq!(result.summary.total_removed, hunk_rem);
    assert_eq!(result.summary.total_added, hunk_add);
}

// ─── empty inputs ─────────────────────────────────────────────────────────────

#[test]
fn test_empty_to_content() {
    let result = compute_diff("f.rs", "", "line1\nline2\n", None);
    assert_eq!(result.summary.total_added, 2);
    assert_eq!(result.summary.total_removed, 0);
}

#[test]
fn test_content_to_empty() {
    let result = compute_diff("f.rs", "line1\nline2\n", "", None);
    assert_eq!(result.summary.total_removed, 2);
    assert_eq!(result.summary.total_added, 0);
}
