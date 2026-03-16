// src/main.rs
//
// seeyue-mcp: Windows-native MCP Server for seeyue-workflows
// 传输层：stdio（JSON-RPC 2.0）
// SDK：rmcp v1.2.0
//
// ToolError is intentionally large (~192 bytes) for rich error context.
// Suppress clippy::result_large_err crate-wide until a Box refactor is warranted.
#![allow(clippy::result_large_err)]
// 协议版本：MCP 2025-06-18（rmcp 当前协商版本）
//
// P0: File editing tools (read/write/edit/multi_edit/rewind)
// P1: Policy engine + hook tools + workflow resources

mod backup;
mod cache;
mod checkpoint;
mod diff;
mod encoding;
mod error;
mod platform;
mod policy;
mod resources;
mod tools;
mod workflow;

use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use anyhow::Result;

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

use backup::{BackupManager, BackupConfig};
use cache::ReadCache;
use checkpoint::CheckpointStore;
use error::ToolError;
use policy::evaluator::PolicyEngine;
use policy::spec_loader::PolicySpecs;

// ─── 共享状态 ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    // P0
    pub workspace:     Arc<PathBuf>,
    pub cache:         Arc<RwLock<ReadCache>>,
    pub checkpoint:    Arc<CheckpointStore>,
    pub backup:        Arc<BackupManager>,
    // P1
    pub workflow_dir:  PathBuf,
    pub policy_engine: Arc<PolicyEngine>,
}

// ─── MCP Server ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SeeyueMcpServer {
    state:       AppState,
    tool_router: ToolRouter<SeeyueMcpServer>,
}

// ─── 工具参数结构体（P0）────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadFileParams {
    #[schemars(description = "File path relative to workspace root (forward or back slashes both ok)")]
    file_path:  String,
    #[schemars(description = "Start line, 1-based (default: 1)")]
    start_line: Option<u32>,
    #[schemars(description = "End line inclusive (default: EOF). Max 2000 lines per call.")]
    end_line:   Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WriteParams {
    #[schemars(description = "File path relative to workspace root")]
    file_path: String,
    #[schemars(description = "Complete file content. Encoding and line endings are preserved on overwrite.")]
    content:   String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EditParams {
    #[schemars(description = "File path relative to workspace root")]
    file_path:  String,
    #[schemars(description = "Exact string to replace. Copy verbatim from read_file output — tabs are \\t, not spaces.")]
    old_string: String,
    #[schemars(description = "Replacement string. Empty string = delete old_string.")]
    new_string: String,
    #[schemars(description = "Replace all occurrences (default: false — fail if multiple matches)")]
    replace_all: Option<bool>,
    #[schemars(description = "Skip cache freshness check (default: false)")]
    force:      Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SingleEdit {
    old_string:  String,
    new_string:  String,
    replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MultiEditParams {
    #[schemars(description = "File path relative to workspace root")]
    file_path: String,
    #[schemars(description = "Ordered list of edits to apply atomically")]
    edits:     Vec<SingleEdit>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RewindParams {
    #[schemars(description = "Number of write operations to undo (default: 1)")]
    steps: Option<u32>,
}

// ─── tool_router impl ────────────────────────────────────────────────────────

#[tool_router]
impl SeeyueMcpServer {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    // ── P0 Tools ─────────────────────────────────────────────────────────

    #[tool(description = "\
        Read a file from the workspace. \
        Returns raw content with tabs preserved as \\t (never converted to spaces). \
        Line endings reported but not altered. \
        Max 2000 lines per call; use start_line/end_line for large files.")]
    async fn read_file(
        &self,
        Parameters(p): Parameters<ReadFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let cache = self.state.cache.read().await;
        tools::read::run_read(
            tools::read::ReadParams {
                file_path:  p.file_path,
                start_line: p.start_line.map(|v| v as usize),
                end_line:   p.end_line.map(|v| v as usize),
            },
            &cache,
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Write complete file content to workspace. \
        Requires read_file first (cache freshness check). \
        Preserves original encoding (UTF-8/GBK/Shift-JIS/UTF-16LE), BOM, and line endings (CRLF/LF). \
        Creates parent directories automatically.")]
    async fn write(
        &self,
        Parameters(p): Parameters<WriteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let call_id = format!("write_{}", chrono::Utc::now().timestamp_millis());
        let cache = self.state.cache.read().await;
        tools::write::run_write(
            tools::write::WriteParams {
                file_path: p.file_path,
                content:   p.content,
            },
            &cache,
            &self.state.checkpoint,
            &self.state.backup,
            &self.state.workspace,
            &call_id,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Replace exact string in file. \
        Three-level match fallback: exact bytes → tab/space normalization → Unicode confusion detection. \
        Creates a Checkpoint snapshot before writing (use rewind to undo). \
        old_string must match verbatim including \\t for tabs.")]
    async fn edit(
        &self,
        Parameters(p): Parameters<EditParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let call_id = format!("edit_{}", chrono::Utc::now().timestamp_millis());
        let cache = self.state.cache.read().await;
        tools::edit::run_edit(
            tools::edit::EditParams {
                file_path:  p.file_path,
                old_string: p.old_string,
                new_string: p.new_string,
                replace_all: p.replace_all.unwrap_or(false),
                force:       p.force.unwrap_or(false),
            },
            &cache,
            &self.state.checkpoint,
            &self.state.backup,
            &self.state.workspace,
            &call_id,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Apply multiple string replacements to one file atomically. \
        All edits are validated first — if any edit fails, the file is unchanged. \
        Edits are applied in order; later edits see the result of earlier ones. \
        One Checkpoint snapshot per call.")]
    async fn multi_edit(
        &self,
        Parameters(p): Parameters<MultiEditParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let call_id = format!("multi_edit_{}", chrono::Utc::now().timestamp_millis());
        let cache = self.state.cache.read().await;
        let edits = p.edits.into_iter().map(|e| tools::edit::SingleEdit {
            old_string:  e.old_string,
            new_string:  e.new_string,
            replace_all: e.replace_all.unwrap_or(false),
            expected_replacements: None,
        }).collect();
        tools::edit::run_multi_edit(
            tools::edit::MultiEditParams {
                file_path: p.file_path,
                edits,
            },
            &cache,
            &self.state.checkpoint,
            &self.state.backup,
            &self.state.workspace,
            &call_id,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Undo the last N write operations using pre-write Checkpoints (SQLite snapshots). \
        Checkpoints are independent of git — works even in non-git directories. \
        Checkpoints are cleared at session end.")]
    async fn rewind(
        &self,
        Parameters(p): Parameters<RewindParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let steps = p.steps.unwrap_or(1) as usize;
        self.state.checkpoint.rewind(steps)
            .map(|paths| {
                let msg = if paths.is_empty() {
                    "No checkpoints to rewind.".to_string()
                } else {
                    format!(
                        "Rewound {n} file(s):\n{list}",
                        n    = paths.len(),
                        list = paths.iter()
                            .map(|p| format!("  - {}", p.display()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                };
                to_text(msg)
            })
            .map_err(to_mcp_err)
    }

    // ── P1 Hook Tools ────────────────────────────────────────────────────

    #[tool(description = "\
        [Hook] Pre-tool check for Bash commands. \
        Classifies command (destructive/privileged/git_mutating/network_sensitive/etc), \
        checks approval matrix, loop budget, and git special rules. \
        Returns verdict: allow, block, or block_with_approval_request.")]
    async fn sy_pretool_bash(
        &self,
        Parameters(p): Parameters<tools::hooks::PreToolBashParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::hooks::run_pretool_bash(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Pre-tool check for Write/Edit operations. \
        Classifies file (secret_material/security_boundary/system_file/etc), \
        checks approval matrix, TDD state, and scope drift. \
        Returns verdict: allow, block, or block_with_approval_request.")]
    async fn sy_pretool_write(
        &self,
        Parameters(p): Parameters<tools::hooks::PreToolWriteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::hooks::run_pretool_write(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Post-tool evidence capture for Write/Edit operations. \
        Records write event to journal.jsonl for audit trail. \
        Always returns allow.")]
    async fn sy_posttool_write(
        &self,
        Parameters(p): Parameters<tools::hooks::PostToolWriteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::hooks::run_posttool_write(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Stop gate check. \
        Verifies loop budget, pending approvals, and restore state. \
        Returns allow or force_continue (prevent premature stop).")]
    async fn sy_stop(
        &self,
        Parameters(p): Parameters<tools::hooks::StopParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::hooks::run_stop(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        Create a named checkpoint with optional file snapshots. \
        Records event to journal. Snapshots can be restored with rewind.")]
    async fn sy_create_checkpoint(
        &self,
        Parameters(p): Parameters<tools::hooks::CreateCheckpointParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::hooks::run_create_checkpoint(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        Advance workflow to a new node. \
        Updates session.yaml with new node info (id, name, status, TDD state, targets). \
        Records node_exited and node_entered events to journal.")]
    async fn sy_advance_node(
        &self,
        Parameters(p): Parameters<tools::hooks::AdvanceNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::hooks::run_advance_node(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }
}

// ─── ServerHandler ────────────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for SeeyueMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build()
        )
        .with_instructions(
            "seeyue-mcp: Windows-native file editing + workflow policy engine. \
             P0 Tools: read_file, write, edit, multi_edit, rewind — always read before edit/write. \
             P1 Hook Tools: sy_pretool_bash, sy_pretool_write, sy_posttool_write, sy_stop, \
             sy_create_checkpoint, sy_advance_node — call these for policy decisions. \
             Resources: workflow://session, workflow://task-graph, workflow://journal."
            .to_string()
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: resources::workflow::list_resources(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        resources::workflow::read_resource(&request.uri, &self.state.workflow_dir)
            .map_err(|e| ErrorData::invalid_params(e, None))
    }
}

// ─── 入口 ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Windows: 启用 ANSI 颜色（ENABLE_VIRTUAL_TERMINAL_PROCESSING），MCP stdout 保持干净
    platform::terminal::init();

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
            // Return empty specs — all commands/files default to safe/workspace
            PolicySpecs::load_empty()
        });

    let state = AppState {
        workspace:     Arc::new(workspace.clone()),
        cache:         Arc::new(RwLock::new(ReadCache::new())),
        checkpoint:    Arc::new(CheckpointStore::open(&session_id, &db_dir)
            .map_err(|e| anyhow::anyhow!("{:?}", e))?),
        backup:        Arc::new(BackupManager::new(
            BackupConfig {
                directory: workspace.join(".agent-backups"),
                ..BackupConfig::default()
            },
            session_id,
        )),
        workflow_dir,
        policy_engine: Arc::new(PolicyEngine::new(policy_specs)),
    };

    let server = SeeyueMcpServer::new(state);

    // MCP over stdio：Claude Code / Gemini CLI / Cursor 均使用此传输层
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}

// ─── 转换辅助 ─────────────────────────────────────────────────────────────────

fn to_text(s: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(s)])
}

fn to_mcp_err(e: ToolError) -> ErrorData {
    ErrorData::invalid_params(e.to_json(), None)
}
