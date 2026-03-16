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
mod git;
mod lsp;
mod platform;
mod policy;
mod prompts;
mod resources;
mod treesitter;
mod tools;
mod workflow;

use std::{path::PathBuf, sync::{Arc, Mutex}};
use tokio::sync::RwLock;
use anyhow::Result;

use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{
        router::prompt::PromptRouter,
        router::tool::ToolRouter,
        wrapper::Parameters,
    },
    model::*,
    schemars,
    prompt_handler, prompt_router,
    tool, tool_handler, tool_router,
    service::RequestContext,
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
    // P2
    pub lsp_pool:      Arc<Mutex<lsp::LspSessionPool>>,
    pub skill_registry: Arc<prompts::SkillRegistry>,
}

// ─── MCP Server ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SeeyueMcpServer {
    state:         AppState,
    tool_router:   ToolRouter<SeeyueMcpServer>,
    prompt_router: PromptRouter<SeeyueMcpServer>,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ResolvePathParams {
    #[schemars(description = "Any path form (forward/back slashes, .., ~). Returned as normalized absolute path.")]
    path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
struct EnvInfoParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FileOutlineParams {
    #[schemars(description = "File path relative to workspace root")]
    path:  String,
    #[schemars(description = "Outline depth: 0=top-level, 1=include methods (default), 2=all descendants")]
    depth: Option<u8>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct VerifySyntaxParams {
    #[schemars(description = "File path relative to workspace root (optional if content is provided)")]
    path:     Option<String>,
    #[schemars(description = "Source content to verify (optional if path is provided)")]
    content:  Option<String>,
    #[schemars(description = "Language hint when content is provided (rust/python/typescript/tsx/go)")]
    language: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadRangeParams {
    #[schemars(description = "File path relative to workspace root")]
    path:          String,
    #[schemars(description = "Start line (1-based)")]
    start:         Option<usize>,
    #[schemars(description = "End line (1-based)")]
    end:           Option<usize>,
    #[schemars(description = "Symbol name to resolve range from file_outline")]
    symbol:        Option<String>,
    #[schemars(description = "Context lines to include above and below")]
    context_lines: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchWorkspaceParams {
    #[schemars(description = "Search pattern (regex or literal)")]
    pattern:       String,
    #[schemars(description = "Whether pattern is a regex (default: false)")]
    is_regex:      Option<bool>,
    #[schemars(description = "Optional file glob filter (e.g., src/**/*.rs)")]
    file_glob:     Option<String>,
    #[schemars(description = "Context lines to include above and below")]
    context_lines: Option<usize>,
    #[schemars(description = "Max results to return (default: 50)")]
    max_results:   Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WorkspaceTreeParams {
    #[schemars(description = "Max directory depth (default: 3)")]
    depth:             Option<usize>,
    #[schemars(description = "Respect .gitignore/.ignore (default: true)")]
    respect_gitignore: Option<bool>,
    #[schemars(description = "Show hidden files (default: false)")]
    show_hidden:       Option<bool>,
    #[schemars(description = "Minimum file size in bytes to include (default: 0)")]
    min_size_bytes:    Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadCompressedParams {
    #[schemars(description = "File path relative to workspace root")]
    path:         String,
    #[schemars(description = "Target token budget (default: 800)")]
    token_budget: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PreviewEditParams {
    #[schemars(description = "File path relative to workspace root")]
    file_path:  String,
    #[schemars(description = "Exact string to replace")]
    old_string: String,
    #[schemars(description = "Replacement string")]
    new_string: String,
    #[schemars(description = "Replace all occurrences (default: false)")]
    replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FindDefinitionParams {
    #[schemars(description = "File path relative to workspace root")]
    path:   String,
    #[schemars(description = "1-based line number")]
    line:   usize,
    #[schemars(description = "1-based column number")]
    column: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FindReferencesParams {
    #[schemars(description = "File path relative to workspace root")]
    path:   String,
    #[schemars(description = "1-based line number")]
    line:   usize,
    #[schemars(description = "1-based column number")]
    column: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GitStatusParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GitDiffFileParams {
    #[schemars(description = "File path relative to workspace root")]
    path:   String,
    #[schemars(description = "Base git ref (default: HEAD)")]
    base:   Option<String>,
    #[schemars(description = "Use staged version instead of working tree (default: false)")]
    staged: Option<bool>,
}

// ─── tool_router impl ────────────────────────────────────────────────────────

#[tool_router]
impl SeeyueMcpServer {
    pub fn new(state: AppState) -> Self {
        let mut prompt_router = Self::prompt_router();
        prompt_router.merge(prompts::build_prompt_router(state.skill_registry.clone()));
        Self {
            state,
            tool_router: Self::tool_router(),
            prompt_router,
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

    // ── P2 Windows Tools ────────────────────────────────────────────────

    #[tool(description = "\
        Normalize and resolve a Windows path relative to the workspace. \
        Returns absolute + relative paths, existence, and directory status. \
        Rejects path escape outside workspace.")]
    async fn resolve_path(
        &self,
        Parameters(p): Parameters<ResolvePathParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::resolve_path::run_resolve_path(
            tools::resolve_path::ResolvePathParams { path: p.path },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Return workspace and system environment diagnostics for this MCP server. \
        Includes codepage, disk free space, git and rust-analyzer availability.")]
    async fn env_info(
        &self,
        Parameters(_): Parameters<EnvInfoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = tools::env_info::run_env_info(&self.state.workspace);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    // ── P2 tree-sitter Tools ─────────────────────────────────────────────

    #[tool(description = "\
        Return a compact symbol outline of a source file using tree-sitter. \
        Designed to be ~200 tokens and used with read_range for focused reads.")]
    async fn file_outline(
        &self,
        Parameters(p): Parameters<FileOutlineParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::file_outline::run_file_outline(
            tools::file_outline::FileOutlineParams {
                path:  p.path,
                depth: p.depth,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Verify source syntax using tree-sitter. \
        Accepts either file path or direct content. Returns error locations when invalid.")]
    async fn verify_syntax(
        &self,
        Parameters(p): Parameters<VerifySyntaxParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::verify_syntax::run_verify_syntax(
            tools::verify_syntax::VerifySyntaxParams {
                path:     p.path,
                content:  p.content,
                language: p.language,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    // ── P2 Search & Navigation ───────────────────────────────────────────

    #[tool(description = "\
        Read a specific line range, optionally resolved from a symbol name. \
        Supports context lines around the target range.")]
    async fn read_range(
        &self,
        Parameters(p): Parameters<ReadRangeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::read_range::run_read_range(
            tools::read_range::ReadRangeParams {
                path:          p.path,
                start:         p.start,
                end:           p.end,
                symbol:        p.symbol,
                context_lines: p.context_lines,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Search the workspace for a pattern. \
        Respects .gitignore by default and supports regex or literal matching.")]
    async fn search_workspace(
        &self,
        Parameters(p): Parameters<SearchWorkspaceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::search_workspace::run_search_workspace(
            tools::search_workspace::SearchWorkspaceParams {
                pattern:       p.pattern,
                is_regex:      p.is_regex,
                file_glob:     p.file_glob,
                context_lines: p.context_lines,
                max_results:   p.max_results,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Return a directory tree with file metadata (size, language, modified time). \
        Depth-limited and .gitignore-aware.")]
    async fn workspace_tree(
        &self,
        Parameters(p): Parameters<WorkspaceTreeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::workspace_tree::run_workspace_tree(
            tools::workspace_tree::WorkspaceTreeParams {
                depth:             p.depth,
                respect_gitignore: p.respect_gitignore,
                show_hidden:       p.show_hidden,
                min_size_bytes:    p.min_size_bytes,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    // ── P2 Advanced Tools ─────────────────────────────────────────────────

    #[tool(description = "\
        Read a file with progressive compression to fit a token budget. \
        Applies up to 4 compression levels (blank lines → comments → imports → skeleton).")]
    async fn read_compressed(
        &self,
        Parameters(p): Parameters<ReadCompressedParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::read_compressed::run_read_compressed(
            tools::read_compressed::ReadCompressedParams {
                path: p.path,
                token_budget: p.token_budget,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Preview an edit without writing to disk. \
        Returns diff and syntax validation result.")]
    async fn preview_edit(
        &self,
        Parameters(p): Parameters<PreviewEditParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::preview_edit::run_preview_edit(
            tools::preview_edit::PreviewEditParams {
                file_path: p.file_path,
                old_string: p.old_string,
                new_string: p.new_string,
                replace_all: p.replace_all,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Find the definition of the symbol at a given position. \
        Uses LSP with a 3s timeout and falls back to grep.")]
    async fn find_definition(
        &self,
        Parameters(p): Parameters<FindDefinitionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::find_definition::run_find_definition(
            tools::find_definition::FindDefinitionParams {
                path: p.path,
                line: p.line,
                column: p.column,
            },
            &self.state,
        )
        .await
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Find references to the symbol at a given position. \
        Uses LSP with a 3s timeout and falls back to grep.")]
    async fn find_references(
        &self,
        Parameters(p): Parameters<FindReferencesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::find_references::run_find_references(
            tools::find_references::FindReferencesParams {
                path: p.path,
                line: p.line,
                column: p.column,
            },
            &self.state,
        )
        .await
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    // ── P2 Git Tools ─────────────────────────────────────────────────────

    #[tool(description = "\
        Return a structured git status summary for the workspace. \
        Includes modified, added, deleted, staged, untracked, and conflict paths.")]
    async fn git_status(
        &self,
        Parameters(_): Parameters<GitStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::git_status::run_git_status(&self.state.workspace)
            .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
            .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Return a structured diff for a single file against a git base ref. \
        Use staged=true to compare the index version instead of working tree.")]
    async fn git_diff_file(
        &self,
        Parameters(p): Parameters<GitDiffFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::git_diff_file::run_git_diff_file(
            tools::git_diff_file::GitDiffFileParams {
                path: p.path,
                base: p.base,
                staged: p.staged,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }
}

// ─── prompt_router impl ────────────────────────────────────────────────────

#[prompt_router]
impl SeeyueMcpServer {}

// ─── ServerHandler ────────────────────────────────────────────────────────────

#[tool_handler]
#[prompt_handler]
impl ServerHandler for SeeyueMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build()
        )
        .with_instructions(
            "seeyue-mcp: Windows-native file editing + workflow policy engine. \
             P0 Tools: read_file, write, edit, multi_edit, rewind — always read before edit/write. \
             P1 Hook Tools: sy_pretool_bash, sy_pretool_write, sy_posttool_write, sy_stop, \
             sy_create_checkpoint, sy_advance_node — call these for policy decisions. \
             P2 Prompts: skills registry via prompts/list and prompts/get. \
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

    let skill_registry = prompts::SkillRegistry::load(&workspace)
        .unwrap_or_else(|e| {
            eprintln!("[seeyue-mcp] Warning: failed to load skills.spec.yaml: {}", e);
            prompts::SkillRegistry::load_empty(&workspace)
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
        lsp_pool:      Arc::new(Mutex::new(lsp::LspSessionPool::new())),
        skill_registry: Arc::new(skill_registry),
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
