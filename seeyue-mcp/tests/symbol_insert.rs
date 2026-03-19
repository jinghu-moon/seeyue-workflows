// tests/symbol_insert.rs
//
// TDD tests for sy_insert_after_symbol and sy_insert_before_symbol (B-N3).
// Run: cargo test --test symbol_insert

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
use seeyue_mcp::tools::insert_symbol::{run_insert_after_symbol, run_insert_before_symbol, InsertSymbolParams};

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

const SRC: &str = "pub fn alpha() -> i32 { 1 }

pub fn beta() -> i32 { 2 }
";

// B-N3 test 1: insert_after — new content appears after the symbol's end_line
#[tokio::test]
async fn test_insert_after_symbol() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), SRC).unwrap();

    let state = make_state(dir.path());
    let params = InsertSymbolParams {
        name_path:     "alpha".into(),
        relative_path: "lib.rs".into(),
        content:       "\npub fn alpha_extra() -> i32 { 99 }\n".into(),
    };
    run_insert_after_symbol(params, &state).await.unwrap();
    let result = fs::read_to_string(dir.path().join("lib.rs")).unwrap();
    // alpha_extra should appear after alpha and before beta
    let pos_alpha = result.find("alpha()").unwrap();
    let pos_extra = result.find("alpha_extra").unwrap();
    let pos_beta  = result.find("beta()").unwrap();
    assert!(pos_alpha < pos_extra, "alpha_extra should come after alpha");
    assert!(pos_extra < pos_beta,  "alpha_extra should come before beta");
}

// B-N3 test 2: insert_before — new content appears before the symbol's start_line
#[tokio::test]
async fn test_insert_before_symbol() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), SRC).unwrap();

    let state = make_state(dir.path());
    let params = InsertSymbolParams {
        name_path:     "beta".into(),
        relative_path: "lib.rs".into(),
        content:       "pub fn before_beta() -> i32 { 0 }\n".into(),
    };
    run_insert_before_symbol(params, &state).await.unwrap();
    let result = fs::read_to_string(dir.path().join("lib.rs")).unwrap();
    let pos_before = result.find("before_beta").unwrap();
    let pos_beta   = result.find("beta()").unwrap();
    assert!(pos_before < pos_beta, "before_beta should appear before beta");
}

// B-N3 test 3: symbol not found → error
#[tokio::test]
async fn test_insert_after_symbol_not_found() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), SRC).unwrap();

    let state = make_state(dir.path());
    let params = InsertSymbolParams {
        name_path:     "nonexistent".into(),
        relative_path: "lib.rs".into(),
        content:       "fn x() {}\n".into(),
    };
    let result = run_insert_after_symbol(params, &state).await;
    assert!(result.is_err(), "should error for missing symbol");
}

// B-N3 test 4: insert_after last symbol — content appended at end of file
#[tokio::test]
async fn test_insert_after_last_symbol() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), SRC).unwrap();

    let state = make_state(dir.path());
    let params = InsertSymbolParams {
        name_path:     "beta".into(),
        relative_path: "lib.rs".into(),
        content:       "pub fn gamma() -> i32 { 3 }\n".into(),
    };
    run_insert_after_symbol(params, &state).await.unwrap();
    let result = fs::read_to_string(dir.path().join("lib.rs")).unwrap();
    let pos_beta  = result.find("beta()").unwrap();
    let pos_gamma = result.find("gamma()").unwrap();
    assert!(pos_gamma > pos_beta, "gamma should appear after beta");
}

// B-N3 test 5: atomic write — .tmp file cleaned up
#[tokio::test]
async fn test_insert_atomic_write() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), SRC).unwrap();

    let state = make_state(dir.path());
    let params = InsertSymbolParams {
        name_path:     "alpha".into(),
        relative_path: "lib.rs".into(),
        content:       "// inserted\n".into(),
    };
    run_insert_after_symbol(params, &state).await.unwrap();
    // .tmp file should not remain
    assert!(!dir.path().join("lib.rs.tmp").exists(), ".tmp file should be cleaned up");
}
