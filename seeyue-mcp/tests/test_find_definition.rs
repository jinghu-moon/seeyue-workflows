// tests/test_find_definition.rs
//
// Tests for tools::find_definition::run_find_definition.
// Run: cargo test --test test_find_definition

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
use seeyue_mcp::tools::find_definition::{FindDefinitionParams, run_find_definition};

fn make_state(workspace: &std::path::Path) -> AppState {
    let specs = PolicySpecs::load(workspace).unwrap_or_else(|_| PolicySpecs::load(workspace).unwrap());
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

fn ws() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[tokio::test]
async fn test_find_definition_file_not_found_errors() {
    let state = make_state(&ws());
    let err = run_find_definition(
        FindDefinitionParams { path: "src/does_not_exist_xyz.rs".into(), line: 1, column: 1 },
        &state,
    ).await.unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("FileNotFound") || msg.contains("NotFound"), "unexpected: {msg}");
}

#[tokio::test]
async fn test_find_definition_path_escape_blocked() {
    let state = make_state(&ws());
    let err = run_find_definition(
        FindDefinitionParams { path: "../../outside.rs".into(), line: 1, column: 1 },
        &state,
    ).await.unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("PathEscape") || msg.contains("outside"), "unexpected: {msg}");
}

#[tokio::test]
async fn test_find_definition_returns_result_struct() {
    let state = make_state(&ws());
    // main.rs exists — even if LSP is unavailable, grep fallback should return a result
    let result = run_find_definition(
        FindDefinitionParams { path: "src/main.rs".into(), line: 10, column: 5 },
        &state,
    ).await.unwrap();
    assert_eq!(result.kind, "success");
}

#[tokio::test]
async fn test_find_definition_definitions_accessible() {
    let state = make_state(&ws());
    let result = run_find_definition(
        FindDefinitionParams { path: "src/main.rs".into(), line: 10, column: 5 },
        &state,
    ).await.unwrap();
    let _ = result.definitions.len();
}

#[tokio::test]
async fn test_find_definition_symbol_accessible() {
    let state = make_state(&ws());
    let result = run_find_definition(
        FindDefinitionParams { path: "src/main.rs".into(), line: 10, column: 5 },
        &state,
    ).await.unwrap();
    let _ = result.symbol.len();
}
