// tests/test_memory_delete.rs
use seeyue_mcp::tools::memory_delete::{MemoryDeleteParams, run_memory_delete};
use seeyue_mcp::tools::memory_write::{MemoryWriteParams, run_memory_write};

fn write_entry(workspace: &std::path::Path, key: &str) {
    run_memory_write(
        MemoryWriteParams {
            key:     key.into(),
            content: "test content".into(),
            tags:    vec![],
            mode:    None,
        },
        workspace,
    ).unwrap();
}

#[test]
fn test_delete_existing_key() {
    let tmp = tempfile::tempdir().unwrap();
    write_entry(tmp.path(), "foo/bar");
    let result = run_memory_delete(
        MemoryDeleteParams { key: "foo/bar".into() },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "deleted");
    assert_eq!(result.key, "foo/bar");
    // File should be gone
    assert!(!tmp.path().join(".ai/memory/foo/bar.md").exists());
}

#[test]
fn test_delete_missing_key_returns_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let result = run_memory_delete(
        MemoryDeleteParams { key: "nonexistent".into() },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "not_found");
}

#[test]
fn test_delete_removes_from_index() {
    let tmp = tempfile::tempdir().unwrap();
    write_entry(tmp.path(), "decisions/arch");
    run_memory_delete(
        MemoryDeleteParams { key: "decisions/arch".into() },
        tmp.path(),
    ).unwrap();
    let index_path = tmp.path().join(".ai/memory/index.json");
    let raw = std::fs::read_to_string(&index_path).unwrap();
    let index: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(index.get("decisions/arch").is_none());
}
