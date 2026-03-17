// src/app_state.rs
//
// Shared runtime state, extracted from main.rs so that lib.rs can expose it
// for integration tests and tool modules.

use std::{path::PathBuf, sync::{Arc, Mutex}};
use tokio::sync::RwLock;

use crate::lsp;
use crate::policy::evaluator::PolicyEngine;
use crate::prompts::SkillRegistry;
use crate::storage::backup::BackupManager;
use crate::storage::cache::ReadCache;
use crate::storage::checkpoint::CheckpointStore;

#[derive(Clone)]
pub struct AppState {
    // P0
    pub workspace:      Arc<PathBuf>,
    pub cache:          Arc<RwLock<ReadCache>>,
    pub checkpoint:     Arc<CheckpointStore>,
    pub backup:         Arc<BackupManager>,
    // P1
    pub workflow_dir:   PathBuf,
    pub policy_engine:  Arc<PolicyEngine>,
    // P2
    pub lsp_pool:       Arc<Mutex<lsp::LspSessionPool>>,
    pub skill_registry: Arc<SkillRegistry>,
}
