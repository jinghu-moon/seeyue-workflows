// tests/test_memory_list.rs
use seeyue_mcp::tools::memory_list::{MemoryListParams, run_memory_list};
use seeyue_mcp::tools::memory_write::{MemoryWriteParams, run_memory_write};

fn write_entry(workspace: &std::path::Path, key: &str, tags: Vec<String>) {
    run_memory_write(
        MemoryWriteParams {
            key:     key.into(),
            content: format!("content for {key}"),
            tags,
            mode:    None,
        },
        workspace,
    ).unwrap();
}

#[test]
fn test_list_empty_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let result = run_memory_list(
        MemoryListParams { tag: None, limit: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "empty");
    assert_eq!(result.total, 0);
}

#[test]
fn test_list_returns_all_entries() {
    let tmp = tempfile::tempdir().unwrap();
    write_entry(tmp.path(), "a", vec![]);
    write_entry(tmp.path(), "b", vec![]);
    let result = run_memory_list(
        MemoryListParams { tag: None, limit: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "success");
    assert_eq!(result.total, 2);
}

#[test]
fn test_list_tag_filter() {
    let tmp = tempfile::tempdir().unwrap();
    write_entry(tmp.path(), "x", vec!["arch".into()]);
    write_entry(tmp.path(), "y", vec!["ops".into()]);
    let result = run_memory_list(
        MemoryListParams { tag: Some("arch".into()), limit: None },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.entries[0].key, "x");
}

#[test]
fn test_list_limit_respected() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..5 {
        write_entry(tmp.path(), &format!("entry{i}"), vec![]);
    }
    let result = run_memory_list(
        MemoryListParams { tag: None, limit: Some(2) },
        tmp.path(),
    ).unwrap();
    assert!(result.entries.len() <= 2);
    assert!(result.truncated);
}
