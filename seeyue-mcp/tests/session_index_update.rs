// tests/session_index_update.rs
//
// TDD tests for A-N7: SessionStart hook triggers incremental index update.
// Tests verify ProjectIndex::update is called and the index is refreshed.
// Run: cargo test --test session_index_update

use std::fs;
use seeyue_mcp::tools::project_index::ProjectIndex;
use seeyue_mcp::hooks::session_start::trigger_index_update;

// A-N7 test 1: trigger_index_update creates index if not present
#[test]
fn test_trigger_creates_index_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    assert!(!dir.path().join(".seeyue/index.json").exists());
    trigger_index_update(dir.path());
    // Allow the background thread to finish
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(dir.path().join(".seeyue/index.json").exists(),
        "index.json should be created after trigger_index_update");
}

// A-N7 test 2: trigger_index_update on existing index performs incremental update
#[test]
fn test_trigger_updates_existing_index() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn original() {}").unwrap();
    ProjectIndex::build(dir.path()).unwrap();

    // Add a new file
    fs::write(dir.path().join("new.rs"), "pub fn added() {}").unwrap();
    trigger_index_update(dir.path());
    std::thread::sleep(std::time::Duration::from_millis(200));

    let idx = ProjectIndex::load(dir.path()).unwrap();
    let has_new = idx.files.contains_key("new.rs");
    assert!(has_new, "index should include new.rs after update");
}

// A-N7 test 3: trigger_index_update does not block (returns immediately)
#[test]
fn test_trigger_returns_immediately() {
    let dir = tempfile::tempdir().unwrap();
    // Create many files to make indexing take non-trivial time
    for i in 0..20 {
        fs::write(
            dir.path().join(format!("file{}.rs", i)),
            format!("pub fn func{}() {{}}", i),
        ).unwrap();
    }
    let start = std::time::Instant::now();
    trigger_index_update(dir.path());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "trigger_index_update should return in <50ms (got {}ms)", elapsed.as_millis()
    );
}
