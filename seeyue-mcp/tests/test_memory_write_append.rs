// tests/test_memory_write_append.rs
use seeyue_mcp::tools::memory_write::{MemoryWriteParams, run_memory_write};

#[test]
fn test_append_mode_concatenates_content() {
    let tmp = tempfile::tempdir().unwrap();
    run_memory_write(
        MemoryWriteParams {
            key:     "log/daily".into(),
            content: "Entry 1".into(),
            tags:    vec![],
            mode:    None,
        },
        tmp.path(),
    ).unwrap();
    let result = run_memory_write(
        MemoryWriteParams {
            key:     "log/daily".into(),
            content: "Entry 2".into(),
            tags:    vec![],
            mode:    Some("append".into()),
        },
        tmp.path(),
    ).unwrap();
    assert_eq!(result.kind, "appended");
    let content = std::fs::read_to_string(
        tmp.path().join(".ai/memory/log/daily.md")
    ).unwrap();
    assert!(content.contains("Entry 1"));
    assert!(content.contains("Entry 2"));
    assert!(content.contains("---"));
}

#[test]
fn test_append_on_nonexistent_creates() {
    let tmp = tempfile::tempdir().unwrap();
    let result = run_memory_write(
        MemoryWriteParams {
            key:     "new/key".into(),
            content: "First entry".into(),
            tags:    vec![],
            mode:    Some("append".into()),
        },
        tmp.path(),
    ).unwrap();
    // append on non-existent → created (not appended)
    assert_eq!(result.kind, "created");
}

#[test]
fn test_overwrite_mode_replaces_content() {
    let tmp = tempfile::tempdir().unwrap();
    run_memory_write(
        MemoryWriteParams {
            key:     "doc/notes".into(),
            content: "Old content".into(),
            tags:    vec![],
            mode:    None,
        },
        tmp.path(),
    ).unwrap();
    run_memory_write(
        MemoryWriteParams {
            key:     "doc/notes".into(),
            content: "New content".into(),
            tags:    vec![],
            mode:    Some("overwrite".into()),
        },
        tmp.path(),
    ).unwrap();
    let content = std::fs::read_to_string(
        tmp.path().join(".ai/memory/doc/notes.md")
    ).unwrap();
    assert!(!content.contains("Old content"));
    assert!(content.contains("New content"));
}
