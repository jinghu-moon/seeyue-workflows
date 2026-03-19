// tests/symbol_overview.rs
//
// TDD tests for sy_get_symbols_overview tool.
// Tests tree-sitter (syntax) fallback path; LSP path requires a live server.
// Run: cargo test --test symbol_overview

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
use seeyue_mcp::tools::get_symbols_overview::{run_get_symbols_overview, GetSymbolsOverviewParams};

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

// A-N3 test 1: syntax fallback for Rust file — source should be "syntax"
#[tokio::test]
async fn test_overview_rust_syntax_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    fs::write(&file, "pub fn hello() {}\npub struct Foo;\n").unwrap();

    let state = make_state(dir.path());
    let params = GetSymbolsOverviewParams {
        relative_path: "lib.rs".into(),
        depth: Some(0),
    };

    let result = run_get_symbols_overview(params, &state).await.unwrap();
    assert_eq!(result.source, "syntax");
    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"hello") || names.contains(&"Foo"),
        "expected hello or Foo, got: {:?}", names
    );
}

// A-N3 test 2: file not found -> error
#[tokio::test]
async fn test_overview_file_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path());
    let params = GetSymbolsOverviewParams {
        relative_path: "nonexistent.rs".into(),
        depth: Some(0),
    };
    let result = run_get_symbols_overview(params, &state).await;
    assert!(result.is_err(), "expected error for missing file");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("FileNotFound") || msg.contains("not found") || msg.contains("nonexistent"),
        "expected FileNotFound error, got: {}", msg
    );
}

// A-N3 test 3: depth=0 returns symbols with no children
#[tokio::test]
async fn test_overview_depth_zero_no_children() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    fs::write(&file, "struct Outer;\nimpl Outer {\n    fn method(&self) {}\n}\n").unwrap();

    let state = make_state(dir.path());
    let params = GetSymbolsOverviewParams {
        relative_path: "lib.rs".into(),
        depth: Some(0),
    };
    let result = run_get_symbols_overview(params, &state).await.unwrap();
    for sym in &result.symbols {
        assert!(
            sym.children.is_empty(),
            "depth=0: symbol '{}' should have no children", sym.name
        );
    }
}

// A-N3 test 4: depth=1 allows children
#[tokio::test]
async fn test_overview_depth_one_includes_children() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    fs::write(&file, "struct MyType;\nimpl MyType {\n    pub fn do_work(&self) -> i32 { 42 }\n}\n").unwrap();

    let state = make_state(dir.path());
    let params = GetSymbolsOverviewParams {
        relative_path: "lib.rs".into(),
        depth: Some(1),
    };
    let result = run_get_symbols_overview(params, &state).await.unwrap();
    let has_children = result.symbols.iter().any(|s| !s.children.is_empty());
    assert!(
        has_children,
        "depth=1: expected at least one symbol with children; symbols: {:?}",
        result.symbols.iter().map(|s| (&s.name, s.children.len())).collect::<Vec<_>>()
    );
}

// A-N3 test 5: unknown language still returns result with source=syntax
#[tokio::test]
async fn test_overview_unknown_language_syntax_source() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("script.cobol");
    fs::write(&file, "IDENTIFICATION DIVISION.\nPROGRAM-ID. HELLO.\n").unwrap();

    let state = make_state(dir.path());
    let params = GetSymbolsOverviewParams {
        relative_path: "script.cobol".into(),
        depth: Some(0),
    };
    let result = run_get_symbols_overview(params, &state).await.unwrap();
    assert_eq!(result.source, "syntax");
}
