// tests/test_read_write.rs
//
// Functional tests for tools::read::run_read and tools::write::run_write.
// Uses tempfile for isolated workspaces.
// Run: cargo test --test test_read_write

use std::path::Path;

use seeyue_mcp::storage::backup::{BackupConfig, BackupManager};
use seeyue_mcp::storage::cache::ReadCache;
use seeyue_mcp::storage::checkpoint::CheckpointStore;
use seeyue_mcp::tools::read::{ReadParams, run_read};
use seeyue_mcp::tools::write::{WriteParams, run_write};

// ─── Fixtures ─────────────────────────────────────────────────────────────────

struct TestEnv {
    workspace: tempfile::TempDir,
    cache:     ReadCache,
    checkpoint: CheckpointStore,
    backup:    BackupManager,
}

impl TestEnv {
    fn new() -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let cache     = ReadCache::new();
        let checkpoint = CheckpointStore::open("test", workspace.path()).unwrap();
        let backup    = BackupManager::new(
            BackupConfig::default(),
            "test-session".to_string(),
        );
        Self { workspace, cache, checkpoint, backup }
    }

    fn ws(&self) -> &Path { self.workspace.path() }

    fn write_file(&self, name: &str, content: &str) {
        std::fs::write(self.ws().join(name), content).unwrap();
    }

    fn read_params(&self, name: &str) -> ReadParams {
        ReadParams {
            file_path: name.to_string(),
            start_line: None,
            end_line: None,
        }
    }

    fn write_params(&self, name: &str, content: &str) -> WriteParams {
        WriteParams {
            file_path: name.to_string(),
            content: content.to_string(),
        }
    }
}

// ─── run_read ─────────────────────────────────────────────────────────────────

#[test]
fn test_read_existing_file() {
    let env = TestEnv::new();
    env.write_file("hello.txt", "hello world\n");

    let result = run_read(env.read_params("hello.txt"), &env.cache, env.ws()).unwrap();
    assert!(result.content.contains("hello world"));
    assert_eq!(result.total_lines, 1);
}

#[test]
fn test_read_populates_cache() {
    let env = TestEnv::new();
    env.write_file("src.rs", "fn main() {}\n");

    run_read(env.read_params("src.rs"), &env.cache, env.ws()).unwrap();

    let path = env.ws().join("src.rs");
    assert!(env.cache.has_been_read(&path), "cache should be populated after read");
}

#[test]
fn test_read_nonexistent_returns_error() {
    let env = TestEnv::new();
    let result = run_read(env.read_params("missing.txt"), &env.cache, env.ws());
    assert!(result.is_err());
    let msg = format!("{:?}", result.unwrap_err());
    assert!(msg.contains("FileNotFound") || msg.contains("missing"));
}

#[test]
fn test_read_line_range() {
    let env = TestEnv::new();
    env.write_file("lines.txt", "line1\nline2\nline3\nline4\nline5\n");

    let mut params = env.read_params("lines.txt");
    params.start_line = Some(2);
    params.end_line   = Some(3);

    let result = run_read(params, &env.cache, env.ws()).unwrap();
    assert!(result.content.contains("line2"));
    assert!(result.content.contains("line3"));
    assert!(!result.content.contains("line1"));
    assert!(!result.content.contains("line4"));
}

#[test]
fn test_read_multiline_file() {
    let env = TestEnv::new();
    let content = (0..100).map(|i| format!("line {i}\n")).collect::<String>();
    env.write_file("big.txt", &content);

    let result = run_read(env.read_params("big.txt"), &env.cache, env.ws()).unwrap();
    assert_eq!(result.total_lines, 100);
}

#[test]
fn test_read_empty_file() {
    let env = TestEnv::new();
    env.write_file("empty.txt", "");

    let result = run_read(env.read_params("empty.txt"), &env.cache, env.ws()).unwrap();
    assert_eq!(result.total_lines, 0);
    assert!(result.content.is_empty());
}

// ─── run_write ────────────────────────────────────────────────────────────────

#[test]
fn test_write_new_file() {
    let env = TestEnv::new();
    let result = run_write(
        env.write_params("new.txt", "created content"),
        &env.cache, &env.checkpoint, &env.backup, env.ws(), "call-w01",
    ).unwrap();
    assert!(result.bytes > 0);
    let on_disk = std::fs::read_to_string(env.ws().join("new.txt")).unwrap();
    assert_eq!(on_disk, "created content");
}

#[test]
fn test_write_existing_requires_prior_read() {
    let env = TestEnv::new();
    env.write_file("existing.rs", "fn old() {}");

    // Should fail: file exists but not read yet
    let result = run_write(
        env.write_params("existing.rs", "fn new() {}"),
        &env.cache, &env.checkpoint, &env.backup, env.ws(), "call-w02",
    );
    assert!(result.is_err(), "overwriting without read should error");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(msg.contains("FileNotRead") || msg.contains("Read first") || msg.contains("read"));
}

#[test]
fn test_write_after_read_succeeds() {
    let env = TestEnv::new();
    env.write_file("target.rs", "fn original() {}");

    // Read first to populate cache
    run_read(env.read_params("target.rs"), &env.cache, env.ws()).unwrap();

    // Now write should succeed
    let result = run_write(
        env.write_params("target.rs", "fn updated() {}"),
        &env.cache, &env.checkpoint, &env.backup, env.ws(), "call-w03",
    ).unwrap();
    assert!(result.bytes > 0);
    let on_disk = std::fs::read_to_string(env.ws().join("target.rs")).unwrap();
    assert_eq!(on_disk, "fn updated() {}");
}

#[test]
fn test_write_creates_parent_dirs() {
    let env = TestEnv::new();
    let result = run_write(
        env.write_params("deep/nested/file.rs", "content"),
        &env.cache, &env.checkpoint, &env.backup, env.ws(), "call-w04",
    ).unwrap();
    assert!(result.bytes > 0);
    assert!(env.ws().join("deep/nested/file.rs").exists());
}

#[test]
fn test_write_new_file_bypasses_cache_check() {
    let env = TestEnv::new();
    // New file (doesn't exist) should NOT require prior read
    let result = run_write(
        env.write_params("brand_new.txt", "fresh content"),
        &env.cache, &env.checkpoint, &env.backup, env.ws(), "call-w05",
    );
    assert!(result.is_ok(), "new file write should not require prior read");
}

#[test]
fn test_write_path_escape_blocked() {
    let env = TestEnv::new();
    // Attempt to write outside workspace
    let result = run_write(
        env.write_params("../escaped.txt", "evil"),
        &env.cache, &env.checkpoint, &env.backup, env.ws(), "call-w06",
    );
    assert!(result.is_err(), "path escape should be blocked");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(msg.contains("PathEscape") || msg.contains("escape") || msg.contains("outside"));
}
