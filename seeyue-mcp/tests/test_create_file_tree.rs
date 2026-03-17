// tests/test_create_file_tree.rs
//
// Tests for tools::create_file_tree::run_create_file_tree.
// Run: cargo test --test test_create_file_tree

use seeyue_mcp::tools::create_file_tree::{CreateFileTreeParams, FileNode, run_create_file_tree};
use seeyue_mcp::storage::checkpoint::CheckpointStore;

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn store(dir: &std::path::Path) -> CheckpointStore {
    CheckpointStore::open("test", &dir.join(".seeyue")).unwrap()
}

#[test]
fn test_create_single_file() {
    let tmp = ws();
    let cp = store(tmp.path());
    let result = run_create_file_tree(
        CreateFileTreeParams {
            base_path: tmp.path().to_str().unwrap().to_string(),
            tree: vec![FileNode { path: "hello.txt".into(), content: Some("hi\n".into()), template: None }],
            overwrite: None,
        },
        &cp,
        tmp.path(),
    ).unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.created, 1);
    assert!(tmp.path().join("hello.txt").exists());
}

#[test]
fn test_create_nested_dirs() {
    let tmp = ws();
    let cp = store(tmp.path());
    let result = run_create_file_tree(
        CreateFileTreeParams {
            base_path: tmp.path().to_str().unwrap().to_string(),
            tree: vec![FileNode { path: "a/b/c.txt".into(), content: Some("nested".into()), template: None }],
            overwrite: None,
        },
        &cp,
        tmp.path(),
    ).unwrap();
    assert_eq!(result.created, 1);
    assert!(tmp.path().join("a/b/c.txt").exists());
}

#[test]
fn test_skip_existing_without_overwrite() {
    let tmp = ws();
    std::fs::write(tmp.path().join("existing.txt"), "original").unwrap();
    let cp = store(tmp.path());
    let result = run_create_file_tree(
        CreateFileTreeParams {
            base_path: tmp.path().to_str().unwrap().to_string(),
            tree: vec![FileNode { path: "existing.txt".into(), content: Some("new".into()), template: None }],
            overwrite: Some(false),
        },
        &cp,
        tmp.path(),
    ).unwrap();
    assert_eq!(result.skipped, 1);
    // File must not be overwritten
    let content = std::fs::read_to_string(tmp.path().join("existing.txt")).unwrap();
    assert_eq!(content, "original");
}

#[test]
fn test_overwrite_existing() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "old").unwrap();
    let cp = store(tmp.path());
    let result = run_create_file_tree(
        CreateFileTreeParams {
            base_path: tmp.path().to_str().unwrap().to_string(),
            tree: vec![FileNode { path: "f.txt".into(), content: Some("new".into()), template: None }],
            overwrite: Some(true),
        },
        &cp,
        tmp.path(),
    ).unwrap();
    assert_eq!(result.status, "ok");
    let content = std::fs::read_to_string(tmp.path().join("f.txt")).unwrap();
    assert_eq!(content, "new");
}

#[test]
fn test_empty_tree_errors() {
    let tmp = ws();
    let cp = store(tmp.path());
    let err = run_create_file_tree(
        CreateFileTreeParams {
            base_path: tmp.path().to_str().unwrap().to_string(),
            tree: vec![],
            overwrite: None,
        },
        &cp,
        tmp.path(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("MissingParameter") || msg.contains("tree"));
}

#[test]
fn test_checkpoint_id_returned() {
    let tmp = ws();
    let cp = store(tmp.path());
    let result = run_create_file_tree(
        CreateFileTreeParams {
            base_path: tmp.path().to_str().unwrap().to_string(),
            tree: vec![FileNode { path: "t.txt".into(), content: None, template: None }],
            overwrite: None,
        },
        &cp,
        tmp.path(),
    ).unwrap();
    assert!(!result.checkpoint_id.is_empty());
}
