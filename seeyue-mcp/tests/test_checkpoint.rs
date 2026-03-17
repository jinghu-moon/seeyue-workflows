// tests/test_checkpoint.rs
//
// Functional tests for CheckpointStore: open, capture, rewind, count, list, cleanup.
// Uses tempfile for isolated SQLite DBs — no filesystem side effects.
// Run: cargo test --test test_checkpoint

use std::path::PathBuf;

use seeyue_mcp::storage::checkpoint::CheckpointStore;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn store_in(dir: &std::path::Path) -> CheckpointStore {
    CheckpointStore::open("test-session", dir).expect("open should succeed")
}

// ─── open ─────────────────────────────────────────────────────────────────────

#[test]
fn test_open_creates_db_file() {
    let tmp = tempfile::tempdir().unwrap();
    let _store = store_in(tmp.path());
    let db = tmp.path().join("test-session.db");
    assert!(db.exists(), "SQLite DB should be created on open");
}

#[test]
fn test_open_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    // Opening the same session twice should not fail
    let _s1 = store_in(tmp.path());
    let _s2 = store_in(tmp.path());
}

#[test]
fn test_open_creates_dir_if_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let db_dir = tmp.path().join("nested").join("deep");
    let _store = CheckpointStore::open("s1", &db_dir).expect("open should create dir");
    assert!(db_dir.exists());
}

// ─── count (empty) ────────────────────────────────────────────────────────────

#[test]
fn test_count_empty_on_open() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());
    assert_eq!(store.list().len(), 0, "new store should have 0 snapshots");
}

// ─── capture ──────────────────────────────────────────────────────────────────

#[test]
fn test_capture_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let file = tmp.path().join("hello.txt");
    std::fs::write(&file, b"original content").unwrap();

    store
        .capture(&file, "call-001", "write")
        .expect("capture should succeed");

    assert_eq!(store.list().len(), 1, "count should be 1 after capture");
}

#[test]
fn test_capture_nonexistent_file_stores_null() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let ghost = tmp.path().join("ghost.txt"); // does not exist
    store
        .capture(&ghost, "call-002", "write")
        .expect("capture of non-existent file should succeed (NULL content)");

    assert_eq!(store.list().len(), 1);
}

#[test]
fn test_capture_multiple_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    for i in 0..3u8 {
        let f = tmp.path().join(format!("file{i}.txt"));
        std::fs::write(&f, vec![i; 8]).unwrap();
        store.capture(&f, &format!("call-{i:03}"), "write").unwrap();
    }

    assert_eq!(store.list().len(), 3);
}

// ─── read_snapshot ────────────────────────────────────────────────────────────

#[test]
fn test_read_snapshot_returns_original_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let file = tmp.path().join("data.bin");
    let original = b"snapshot bytes";
    std::fs::write(&file, original).unwrap();

    store.capture(&file, "snap-001", "edit").unwrap();

    // Overwrite file — snapshot should still hold original
    std::fs::write(&file, b"modified content").unwrap();

    let bytes = store
        .read_snapshot("snap-001", &file)
        .expect("read_snapshot should return stored bytes");
    assert_eq!(bytes, original);
}

#[test]
fn test_read_snapshot_not_found_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());
    let file = tmp.path().join("any.txt");
    let result = store.read_snapshot("nonexistent-call", &file);
    assert!(result.is_err(), "missing snapshot should return error");
}

// ─── rewind ────────────────────────────────────────────────────────────────────

#[test]
fn test_rewind_restores_file_content() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let file = tmp.path().join("src.rs");
    std::fs::write(&file, b"fn original() {}").unwrap();

    store.capture(&file, "c-001", "write").unwrap();

    // Simulate a write that changes the file
    std::fs::write(&file, b"fn modified() {}").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"fn modified() {}");

    let restored = store.rewind(1).expect("rewind should succeed");

    assert_eq!(restored.len(), 1, "rewind should restore 1 file");
    assert_eq!(std::fs::read(&file).unwrap(), b"fn original() {}", "file content should be restored");
}

#[test]
fn test_rewind_deletes_new_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let new_file = tmp.path().join("new_file.rs");
    // Capture before file exists (NULL snapshot)
    store.capture(&new_file, "c-002", "write").unwrap();

    // Now create the file
    std::fs::write(&new_file, b"fn new() {}").unwrap();
    assert!(new_file.exists());

    let restored = store.rewind(1).expect("rewind should succeed");

    assert_eq!(restored.len(), 1);
    assert!(!new_file.exists(), "new file should be deleted on rewind");
}

#[test]
fn test_rewind_empty_store_returns_empty_vec() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let restored = store.rewind(3).expect("rewind on empty store should not error");
    assert!(restored.is_empty(), "empty store rewind should return empty vec");
}

#[test]
fn test_rewind_consumes_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let file = tmp.path().join("x.txt");
    std::fs::write(&file, b"v1").unwrap();
    store.capture(&file, "c-x", "write").unwrap();
    assert_eq!(store.list().len(), 1);

    store.rewind(1).unwrap();
    assert_eq!(store.list().len(), 0, "rewound snapshot should be deleted");
}

#[test]
fn test_rewind_multiple_steps() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    // Capture 3 different files
    let paths: Vec<PathBuf> = (0..3)
        .map(|i| {
            let p = tmp.path().join(format!("f{i}.txt"));
            std::fs::write(&p, format!("content-{i}")).unwrap();
            store.capture(&p, &format!("c-{i:03}"), "write").unwrap();
            // Overwrite
            std::fs::write(&p, b"overwritten").unwrap();
            p
        })
        .collect();

    let restored = store.rewind(3).expect("rewind 3 should succeed");
    assert_eq!(restored.len(), 3);

    for (i, p) in paths.iter().enumerate() {
        let expected = format!("content-{i}");
        let actual = std::fs::read_to_string(p).unwrap();
        assert_eq!(actual, expected, "file {i} should be restored");
    }
}

// ─── list ─────────────────────────────────────────────────────────────────────

#[test]
fn test_list_returns_snapshot_info() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    let file = tmp.path().join("listed.rs");
    std::fs::write(&file, b"data").unwrap();
    store.capture(&file, "list-001", "multi_edit").unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].tool_name, "multi_edit");
    assert!(list[0].captured_at_ms > 0);
}

#[test]
fn test_list_empty_on_new_store() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());
    assert!(store.list().is_empty());
}

// ─── cleanup ──────────────────────────────────────────────────────────────────

#[test]
fn test_cleanup_removes_all_snapshots() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_in(tmp.path());

    for i in 0..5u8 {
        let f = tmp.path().join(format!("f{i}.txt"));
        std::fs::write(&f, [i; 4]).unwrap();
        store.capture(&f, &format!("c-{i}"), "write").unwrap();
    }
    assert_eq!(store.list().len(), 5);

    store.cleanup();
    assert_eq!(store.list().len(), 0, "cleanup should remove all snapshots");
}

// ─── clone / shared handle ────────────────────────────────────────────────────

#[test]
fn test_clone_shares_same_db() {
    let tmp = tempfile::tempdir().unwrap();
    let s1 = store_in(tmp.path());
    let s2 = s1.clone();

    let file = tmp.path().join("shared.txt");
    std::fs::write(&file, b"hi").unwrap();
    s1.capture(&file, "clone-001", "write").unwrap();

    assert_eq!(s2.list().len(), 1, "clone should share the same Arc<Mutex<Connection>>");
}
