// tests/test_multi_file_edit.rs
//
// Tests for tools::multi_file_edit::run_multi_file_edit.
// Run: cargo test --test test_multi_file_edit

use seeyue_mcp::tools::multi_file_edit::{FileEditSet, FileEditItem, MultiFileEditParams, run_multi_file_edit};
use seeyue_mcp::storage::checkpoint::CheckpointStore;
use seeyue_mcp::storage::cache::ReadCache;
use seeyue_mcp::storage::backup::{BackupManager, BackupConfig};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn store(dir: &std::path::Path) -> CheckpointStore {
    CheckpointStore::open("test", &dir.join(".seeyue")).unwrap()
}

fn backup(_dir: &std::path::Path) -> BackupManager {
    BackupManager::new(BackupConfig::default(), "test-session".to_string())
}

#[test]
fn test_edit_single_file() {
    let tmp = ws();
    std::fs::write(tmp.path().join("f.txt"), "hello world").unwrap();
    let cp = store(tmp.path());
    let cache = ReadCache::new();
    let bk = backup(tmp.path());
    let result = run_multi_file_edit(
        MultiFileEditParams {
            edits: vec![FileEditSet {
                file_path: "f.txt".into(),
                edits: vec![FileEditItem {
                    old_string: "world".into(),
                    new_string: "rust".into(),
                    replace_all: None,
                }],
            }],
            verify_syntax: None,
        },
        &cache, &cp, &bk, tmp.path(),
    ).unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.files_modified, 1);
    let content = std::fs::read_to_string(tmp.path().join("f.txt")).unwrap();
    assert_eq!(content, "hello rust");
}

#[test]
fn test_edit_multiple_files() {
    let tmp = ws();
    std::fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "bbb").unwrap();
    let cp = store(tmp.path());
    let cache = ReadCache::new();
    let bk = backup(tmp.path());
    let result = run_multi_file_edit(
        MultiFileEditParams {
            edits: vec![
                FileEditSet { file_path: "a.txt".into(), edits: vec![FileEditItem { old_string: "aaa".into(), new_string: "AAA".into(), replace_all: None }] },
                FileEditSet { file_path: "b.txt".into(), edits: vec![FileEditItem { old_string: "bbb".into(), new_string: "BBB".into(), replace_all: None }] },
            ],
            verify_syntax: None,
        },
        &cache, &cp, &bk, tmp.path(),
    ).unwrap();
    assert_eq!(result.files_modified, 2);
}

#[test]
fn test_validation_failure_no_write() {
    let tmp = ws();
    std::fs::write(tmp.path().join("c.txt"), "original").unwrap();
    let cp = store(tmp.path());
    let cache = ReadCache::new();
    let bk = backup(tmp.path());
    // old_string not present → validation_failed
    let result = run_multi_file_edit(
        MultiFileEditParams {
            edits: vec![FileEditSet {
                file_path: "c.txt".into(),
                edits: vec![FileEditItem { old_string: "zzz_not_present".into(), new_string: "x".into(), replace_all: None }],
            }],
            verify_syntax: None,
        },
        &cache, &cp, &bk, tmp.path(),
    );
    // Should either error or return validation_failed
    match result {
        Ok(r) => assert_eq!(r.status, "validation_failed"),
        Err(_) => {} // also acceptable
    }
    // File must be unchanged
    let content = std::fs::read_to_string(tmp.path().join("c.txt")).unwrap();
    assert_eq!(content, "original");
}

#[test]
fn test_empty_edits_errors() {
    let tmp = ws();
    let cp = store(tmp.path());
    let cache = ReadCache::new();
    let bk = backup(tmp.path());
    let err = run_multi_file_edit(
        MultiFileEditParams { edits: vec![], verify_syntax: None },
        &cache, &cp, &bk, tmp.path(),
    ).unwrap_err();
    assert!(format!("{err:?}").contains("MissingParameter") || format!("{err:?}").contains("edits"));
}

#[test]
fn test_checkpoint_id_returned() {
    let tmp = ws();
    std::fs::write(tmp.path().join("d.txt"), "data").unwrap();
    let cp = store(tmp.path());
    let cache = ReadCache::new();
    let bk = backup(tmp.path());
    let result = run_multi_file_edit(
        MultiFileEditParams {
            edits: vec![FileEditSet {
                file_path: "d.txt".into(),
                edits: vec![FileEditItem { old_string: "data".into(), new_string: "new".into(), replace_all: None }],
            }],
            verify_syntax: None,
        },
        &cache, &cp, &bk, tmp.path(),
    ).unwrap();
    assert!(!result.checkpoint_id.is_empty());
}
