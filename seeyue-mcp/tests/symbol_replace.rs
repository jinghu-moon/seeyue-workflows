// tests/symbol_replace.rs
//
// TDD tests for sy_replace_symbol_body (B-N2).
// Run: cargo test --test symbol_replace

use std::fs;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use seeyue_mcp::app_state::AppState;
use seeyue_mcp::lsp::LspSessionPool;
use seeyue_mcp::policy::evaluator::PolicyEngine;
use seeyue_mcp::policy::spec_loader::PolicySpecs;
use seeyue_mcp::prompts::SkillRegistry;
use seeyue_mcp::storage::backup::{BackupManager, BackupConfig};
use seeyue_mcp::storage::cache::ReadCache;
use seeyue_mcp::storage::checkpoint::CheckpointStore;
use seeyue_mcp::tools::replace_symbol_body::{run_replace_symbol_body, ReplaceSymbolBodyParams};

fn make_state(workspace: &std::path::Path) -> AppState {
    let specs = PolicySpecs::load(workspace).unwrap_or_else(|_| PolicySpecs::load_empty());
    AppState {
        workspace:      Arc::new(workspace.to_path_buf()),
        cache:          Arc::new(RwLock::new(ReadCache::new())),
        checkpoint:     Arc::new(CheckpointStore::open("test", &workspace.join(".seeyue")).unwrap()),
        backup:         Arc::new(BackupManager::new(BackupConfig::default(), "test".into())),
        workflow_dir:   workspace.join("workflow"),
        policy_engine:  Arc::new(PolicyEngine::new(specs)),
        lsp_pool:       Arc::new(Mutex::new(LspSessionPool::new())),
        skill_registry: Arc::new(SkillRegistry::load_empty(workspace)),
    }
}

const RUST_SRC: &str = "pub struct Greeter;

impl Greeter {
    pub fn greet(&self) -> String {
        String::from(\"hello\")
    }

    pub fn farewell(&self) -> String {
        String::from(\"bye\")
    }
}
";

// B-N2 test 1: successful replace — content changes and lines_changed correct
#[tokio::test]
async fn test_replace_symbol_body_success() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = ReplaceSymbolBodyParams {
        name_path:     "greet".into(),
        relative_path: "lib.rs".into(),
        new_body:      "    pub fn greet(&self) -> String {\n        String::from(\"hi there\")\n    }".into(),
    };
    let result = run_replace_symbol_body(params, &state).await.unwrap();
    assert!(result.lines_changed > 0, "lines_changed should be > 0");

    let new_content = fs::read_to_string(dir.path().join("lib.rs")).unwrap();
    assert!(new_content.contains("hi there"), "file should contain new body");
    assert!(!new_content.contains("\"hello\""), "old body should be gone");
}

// B-N2 test 2: symbol not found → error
#[tokio::test]
async fn test_replace_symbol_not_found() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn existing() {}\n").unwrap();

    let state = make_state(dir.path());
    let params = ReplaceSymbolBodyParams {
        name_path:     "nonexistent_symbol".into(),
        relative_path: "lib.rs".into(),
        new_body:      "pub fn nonexistent_symbol() {}".into(),
    };
    let result = run_replace_symbol_body(params, &state).await;
    assert!(result.is_err(), "should error when symbol not found");
}

// B-N2 test 3: file not found → error
#[tokio::test]
async fn test_replace_file_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path());
    let params = ReplaceSymbolBodyParams {
        name_path:     "foo".into(),
        relative_path: "missing.rs".into(),
        new_body:      "fn foo() {}".into(),
    };
    let result = run_replace_symbol_body(params, &state).await;
    assert!(result.is_err());
    let msg = format!("{:?}", result.unwrap_err());
    assert!(msg.contains("FileNotFound") || msg.contains("not found"), "got: {}", msg);
}

// B-N2 test 4: farewell method also replaceable (different symbol)
#[tokio::test]
async fn test_replace_second_symbol() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = ReplaceSymbolBodyParams {
        name_path:     "farewell".into(),
        relative_path: "lib.rs".into(),
        new_body:      "    pub fn farewell(&self) -> String {\n        String::from(\"goodbye\")\n    }".into(),
    };
    let result = run_replace_symbol_body(params, &state).await.unwrap();
    assert!(result.lines_changed > 0);
    let content = fs::read_to_string(dir.path().join("lib.rs")).unwrap();
    assert!(content.contains("goodbye"));
    assert!(!content.contains("\"bye\""));
}

// B-N2 test 5: replacement preserves other methods unchanged
#[tokio::test]
async fn test_replace_preserves_other_symbols() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = ReplaceSymbolBodyParams {
        name_path:     "greet".into(),
        relative_path: "lib.rs".into(),
        new_body:      "    pub fn greet(&self) -> String {\n        String::from(\"changed\")\n    }".into(),
    };
    run_replace_symbol_body(params, &state).await.unwrap();
    let content = fs::read_to_string(dir.path().join("lib.rs")).unwrap();
    // farewell should still be present
    assert!(content.contains("farewell"), "farewell method should be preserved");
    assert!(content.contains("\"bye\""), "farewell body should be unchanged");
}
