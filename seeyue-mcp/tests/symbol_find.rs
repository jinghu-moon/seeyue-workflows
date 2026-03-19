// tests/symbol_find.rs
//
// TDD tests for sy_find_symbol tool.
// Run: cargo test --test symbol_find

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
use seeyue_mcp::tools::find_symbol::{run_find_symbol, FindSymbolParams};

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

const RUST_SRC: &str = r#"pub struct UserSession {
    id: u32,
}

impl UserSession {
    pub fn new(id: u32) -> Self { UserSession { id } }
    pub fn validate(&self) -> bool { self.id > 0 }
    pub fn reset(&mut self) { self.id = 0; }
}

pub fn top_level_helper() -> i32 { 42 }
"#;

// A-N4 test 1: exact match by simple name
#[tokio::test]
async fn test_find_symbol_exact_name() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "validate".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(1),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(!result.matches.is_empty(), "expected at least one match for 'validate'");
    let found = result.matches.iter().any(|m| m.name.contains("validate"));
    assert!(found, "expected 'validate' in matches, got: {:?}", result.matches.iter().map(|m| &m.name).collect::<Vec<_>>());
}

// A-N4 test 2: substring_matching=true returns multiple
#[tokio::test]
async fn test_find_symbol_substring_matching() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "e".into(), // matches: UserSession, new, validate, reset, top_level_helper
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(true),
        include_body: Some(false),
        depth: Some(1),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(
        result.matches.len() >= 2,
        "substring 'e' should match multiple symbols, got: {}", result.matches.len()
    );
}

// A-N4 test 3: no match returns empty list
#[tokio::test]
async fn test_find_symbol_no_match_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "xyzzy_does_not_exist".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(1),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(result.matches.is_empty(), "expected no matches");
}

// A-N4 test 4: include_body=true attaches source lines
#[tokio::test]
async fn test_find_symbol_include_body() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), RUST_SRC).unwrap();

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "top_level_helper".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(true),
        depth: Some(0),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(!result.matches.is_empty(), "expected match for top_level_helper");
    let body = result.matches[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("top_level_helper") || body.contains("42"),
        "body should contain function source, got: {:?}", body
    );
}

// A-N4 test 5: relative_path=None searches all rs files in workspace
#[tokio::test]
async fn test_find_symbol_global_search() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "pub fn alpha() {}").unwrap();
    fs::write(dir.path().join("b.rs"), "pub fn beta() {}").unwrap();

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "alpha".into(),
        relative_path: None,
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(0),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(
        !result.matches.is_empty(),
        "global search should find 'alpha' in a.rs"
    );
}
