// src/main.rs
//
// seeyue-mcp: Windows-native MCP Server for seeyue-workflows
// 传输层：stdio（JSON-RPC 2.0）
// SDK：rmcp v1.2.0
//
// ToolError is intentionally large (~192 bytes) for rich error context.
// Suppress clippy::result_large_err crate-wide until a Box refactor is warranted.
#![allow(clippy::result_large_err)]
// MCP Params/Result structs are constructed via serde JSON deserialization at runtime;
// static analysis cannot detect this dynamic construction path.
#![allow(dead_code)]

mod app_state;
mod encoding;
mod error;
mod git;
mod lsp;
mod params;
mod platform;
mod policy;
mod prompts;
mod render;
mod resources;
mod server;
mod storage;
mod treesitter;
mod tools;
mod workflow;

use app_state::AppState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use storage::backup::{BackupManager, BackupConfig};
use storage::cache::ReadCache;
use storage::checkpoint::CheckpointStore;
use policy::evaluator::PolicyEngine;
use policy::spec_loader::PolicySpecs;

#[tokio::main]
async fn main() -> Result<()> {
    // Windows: 启用 ANSI 颜色（ENABLE_VIRTUAL_TERMINAL_PROCESSING），MCP stdout 保持干净
    platform::terminal::init();

    // Windows Toast: 注册自定义 AppUserModelID，确保通知显示 "seeyue-mcp" 应用名
    platform::notify::ensure_registered();

    let workspace = std::env::var("SEEYUE_MCP_WORKSPACE")
        .or_else(|_| std::env::var("AGENT_EDITOR_WORKSPACE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap());

    let session_id = format!("sess_{}", chrono::Utc::now().timestamp_millis());

    // Checkpoint DB 存放于 %LOCALAPPDATA%\seeyue-mcp\checkpoints\
    let db_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("seeyue-mcp")
        .join("checkpoints");

    // P1: workflow directory and policy engine
    let workflow_dir = workspace.join(".ai").join("workflow");
    let policy_specs = PolicySpecs::load(&workspace)
        .unwrap_or_else(|e| {
            eprintln!("[seeyue-mcp] Warning: failed to load policy specs: {}", e);
            eprintln!("[seeyue-mcp] Policy engine will operate in permissive mode.");
            PolicySpecs::load_empty()
        });

    let skill_registry = prompts::SkillRegistry::load(&workspace)
        .unwrap_or_else(|e| {
            eprintln!("[seeyue-mcp] Warning: failed to load skills.spec.yaml: {}", e);
            prompts::SkillRegistry::load_empty(&workspace)
        });

    let state = AppState {
        workspace:      Arc::new(workspace.clone()),
        cache:          Arc::new(RwLock::new(ReadCache::new())),
        checkpoint:     Arc::new(CheckpointStore::open(&session_id, &db_dir)
            .map_err(|e| anyhow::anyhow!("{:?}", e))?),
        backup:         Arc::new(BackupManager::new(
            BackupConfig {
                directory: workspace.join(".agent-backups"),
                ..BackupConfig::default()
            },
            session_id,
        )),
        workflow_dir,
        policy_engine:  Arc::new(PolicyEngine::new(policy_specs)),
        lsp_pool:       Arc::new(Mutex::new(lsp::LspSessionPool::new())),
        skill_registry: Arc::new(skill_registry),
    };

    let server = server::SeeyueMcpServer::new(state);

    // MCP over stdio：Claude Code / Gemini CLI / Cursor 均使用此传输层
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
