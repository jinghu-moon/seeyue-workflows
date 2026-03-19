// tests/project_index.rs
// TDD tests for ProjectIndex (A-N6).
// Run: cargo test --test project_index

use std::fs;
use seeyue_mcp::tools::project_index::ProjectIndex;

// A-N6 test 1: build generates valid JSON file
#[test]
fn test_build_generates_index_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn hello() {}\n").unwrap();

    let idx = ProjectIndex::build(dir.path()).unwrap();
    let index_path = dir.path().join(".seeyue/index.json");
    assert!(index_path.exists(), "index.json should be created");
    let content = fs::read_to_string(&index_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("files").is_some());
    assert!(parsed.get("generated_at").is_some());
    let _ = idx;
}

// A-N6 test 2: load deserializes existing index
#[test]
fn test_load_deserializes_index() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "pub fn foo() {}").unwrap();
    ProjectIndex::build(dir.path()).unwrap();

    let loaded = ProjectIndex::load(dir.path()).unwrap();
    assert!(!loaded.files.is_empty(), "loaded index should have files");
}

// A-N6 test 3: index.json not found returns empty (no panic)
#[test]
fn test_load_missing_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let result = ProjectIndex::load(dir.path());
    assert!(result.is_ok());
    let idx = result.unwrap();
    assert!(idx.files.is_empty());
}

// A-N6 test 4: update only rebuilds changed files
#[test]
fn test_update_only_changes_modified_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("stable.rs"), "pub fn stable() {}").unwrap();
    fs::write(dir.path().join("changing.rs"), "pub fn v1() {}").unwrap();

    let idx1 = ProjectIndex::build(dir.path()).unwrap();
    let mtime_stable_before = idx1.files.get("stable.rs")
        .map(|e| e.mtime).unwrap_or(0);

    // Sleep a bit then modify only changing.rs
    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::write(dir.path().join("changing.rs"), "pub fn v2() {}").unwrap();

    let idx2 = ProjectIndex::update(dir.path()).unwrap();
    let mtime_stable_after = idx2.files.get("stable.rs")
        .map(|e| e.mtime).unwrap_or(0);

    // stable.rs mtime should be unchanged in the index
    assert_eq!(mtime_stable_before, mtime_stable_after,
        "stable.rs mtime should not change after update");
}

// A-N6 test 5: .seeyue/ directory is auto-created
#[test]
fn test_seeyue_dir_auto_created() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("x.rs"), "fn x() {}").unwrap();
    assert!(!dir.path().join(".seeyue").exists());
    ProjectIndex::build(dir.path()).unwrap();
    assert!(dir.path().join(".seeyue").exists());
}

// A-N6 test 6: query returns entries matching name_path
#[test]
fn test_query_finds_symbol() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn greet() {}\npub struct User;\n").unwrap();
    let idx = ProjectIndex::build(dir.path()).unwrap();

    let results = idx.query("greet", false);
    assert!(!results.is_empty(), "expected to find 'greet'");
    assert!(results.iter().any(|e| e.name_path.contains("greet")));
}
