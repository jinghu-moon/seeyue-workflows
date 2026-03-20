// tests/symbol_find_index.rs
//
// TDD tests for A-N4b: sy_find_symbol index.json acceleration layer.
// Run: cargo test --test symbol_find_index

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
use seeyue_mcp::tools::project_index::ProjectIndex;

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

// A-N4b test 1: index exists → find_symbol returns results
#[tokio::test]
async fn test_find_with_index_returns_results() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"),
        "pub struct IndexedType;\nimpl IndexedType {\n    pub fn indexed_method(&self) {}\n}\n"
    ).unwrap();
    ProjectIndex::build(dir.path()).unwrap();
    assert!(dir.path().join(".seeyue/index.json").exists());

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "IndexedType".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(1),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(!result.matches.is_empty(),
        "should find IndexedType when index exists");
}

// A-N4b test 2: index missing → falls back to original path, still returns results
#[tokio::test]
async fn test_find_without_index_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"),
        "pub fn fallback_fn() -> i32 { 0 }\n"
    ).unwrap();
    // Do NOT build index
    assert!(!dir.path().join(".seeyue/index.json").exists());

    let state = make_state(dir.path());
    let params = FindSymbolParams {
        name_path_pattern: "fallback_fn".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(0),
    };
    let result = run_find_symbol(params, &state).await.unwrap();
    assert!(!result.matches.is_empty(),
        "should find fallback_fn even without index");
}

// A-N4b test 3: index hit result consistent with direct path result
#[tokio::test]
async fn test_index_result_consistent_with_direct() {
    let dir = tempfile::tempdir().unwrap();
    let src = "pub fn consistent_fn() {}\n";
    fs::write(dir.path().join("lib.rs"), src).unwrap();

    let state = make_state(dir.path());
    let params_no_idx = FindSymbolParams {
        name_path_pattern: "consistent_fn".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(0),
    };
    let direct = run_find_symbol(params_no_idx, &state).await.unwrap();

    ProjectIndex::build(dir.path()).unwrap();
    let params_with_idx = FindSymbolParams {
        name_path_pattern: "consistent_fn".into(),
        relative_path: Some("lib.rs".into()),
        substring_matching: Some(false),
        include_body: Some(false),
        depth: Some(0),
    };
    let with_idx = run_find_symbol(params_with_idx, &state).await.unwrap();

    assert_eq!(direct.matches.len(), with_idx.matches.len(),
        "index and direct paths should return same number of matches");
}

// A-N4b test 4: index narrows candidate files
// Create 20 files; only 1 contains target_func.
// With index: result should come from exactly 1 file.
// Without index: same result but searches all 20 files.
// Verifies that index-accelerated path returns correct file attribution.
#[tokio::test]
async fn test_index_narrows_candidate_files() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Write 19 files without the target, 1 file with it
    for i in 0..19usize {
        fs::write(src.join(format!("mod{}.rs", i)),
            format!("pub fn noise_fn_{}() {{}}\n", i)).unwrap();
    }
    fs::write(src.join("target.rs"), "pub fn unique_target_xyz() {}\n").unwrap();

    // Build index
    ProjectIndex::build(&ws).unwrap();

    let state = make_state(dir.path());
    let result = run_find_symbol(
        FindSymbolParams {
            name_path_pattern: "unique_target_xyz".into(),
            relative_path: None,
            substring_matching: Some(false),
            include_body: Some(false),
            depth: Some(1),
        },
        &state,
    ).await.unwrap();

    assert_eq!(result.matches.len(), 1, "should find exactly 1 match");
    assert_eq!(result.matches[0].name, "unique_target_xyz");
    // Result must come from target.rs, not any other file
    assert!(
        result.matches[0].file.contains("target"),
        "match should be from target.rs, got: {}", result.matches[0].file
    );
}
