// tests/test_symbol_rename_preview.rs
//
// Tests for tools::symbol_rename_preview::run_symbol_rename_preview.
// Run: cargo test --test test_symbol_rename_preview

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
use seeyue_mcp::tools::symbol_rename_preview::{SymbolRenamePreviewParams, run_symbol_rename_preview};

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

#[test]
fn test_symbol_rename_preview_file_not_found_errors() {
    let state = make_state(&ws());
    let err = run_symbol_rename_preview(
        SymbolRenamePreviewParams {
            path: "src/does_not_exist_xyz.rs".into(),
            line: 1, column: 1,
            new_name: "NewName".into(),
        },
        &state,
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("FileNotFound") || msg.contains("NotFound"), "unexpected: {msg}");
}

#[test]
fn test_symbol_rename_preview_path_escape_blocked() {
    let state = make_state(&ws());
    let err = run_symbol_rename_preview(
        SymbolRenamePreviewParams {
            path: "../../outside.rs".into(),
            line: 1, column: 1,
            new_name: "NewName".into(),
        },
        &state,
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("PathEscape") || msg.contains("outside"), "unexpected: {msg}");
}

#[test]
fn test_symbol_rename_preview_returns_ok_or_lsp_unavailable() {
    let state = make_state(&ws());
    let result = run_symbol_rename_preview(
        SymbolRenamePreviewParams {
            path: "src/main.rs".into(),
            line: 10, column: 5,
            new_name: "NewName".into(),
        },
        &state,
    ).unwrap();
    assert!(
        result.status == "ok" || result.status == "LSP_NOT_AVAILABLE",
        "unexpected status: {}", result.status
    );
}

#[test]
fn test_symbol_rename_preview_dry_run_true() {
    let state = make_state(&ws());
    let result = run_symbol_rename_preview(
        SymbolRenamePreviewParams {
            path: "src/main.rs".into(),
            line: 10, column: 5,
            new_name: "Renamed".into(),
        },
        &state,
    ).unwrap();
    assert!(result.dry_run);
}

#[test]
fn test_symbol_rename_preview_new_name_matches() {
    let state = make_state(&ws());
    let result = run_symbol_rename_preview(
        SymbolRenamePreviewParams {
            path: "src/main.rs".into(),
            line: 10, column: 5,
            new_name: "MyNewName".into(),
        },
        &state,
    ).unwrap();
    assert_eq!(result.new_name, "MyNewName");
}

#[test]
fn test_symbol_rename_preview_affected_files_accessible() {
    let state = make_state(&ws());
    let result = run_symbol_rename_preview(
        SymbolRenamePreviewParams {
            path: "src/main.rs".into(),
            line: 10, column: 5,
            new_name: "MyNewName".into(),
        },
        &state,
    ).unwrap();
    assert_eq!(result.affected_files_count, result.affected_files.len());
}
