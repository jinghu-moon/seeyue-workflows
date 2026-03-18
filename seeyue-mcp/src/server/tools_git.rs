// src/server/tools_git.rs — P2 Git + P3 Execution/Analysis + P4 Extended Tools

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*};
use crate::params::*;
use crate::server::util::{to_text, to_mcp_err};
use super::SeeyueMcpServer;

#[tool_router(router = git_router)]
impl SeeyueMcpServer {
    #[tool(description = "Return a structured git status summary for the workspace.")]
    async fn git_status(
        &self,
        Parameters(_): Parameters<GitStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::git_status::run_git_status(&self.state.workspace)
            .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
            .map_err(to_mcp_err)
    }

    #[tool(description = "Show the diff of a specific file between a git ref and working tree.")]
    async fn git_diff_file(
        &self,
        Parameters(p): Parameters<GitDiffFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::git_diff_file::run_git_diff_file(
            crate::tools::git_diff_file::GitDiffFileParams {
                path: p.path, base: p.base, staged: p.staged,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Structured git commit history. Returns hash/author/date/subject per commit.")]
    async fn git_log(
        &self,
        Parameters(p): Parameters<GitLogParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::git_log::run_git_log(
            crate::tools::git_log::GitLogParams { limit: p.limit, path: p.path, since: p.since },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Per-line authorship via git blame --porcelain.")]
    async fn git_blame(
        &self,
        Parameters(p): Parameters<GitBlameParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::git_blame::run_git_blame(
            crate::tools::git_blame::GitBlameParams {
                path: p.path, start_line: p.start_line, end_line: p.end_line,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Execute a shell command in the workspace. Requires sy_pretool_bash verdict.")]
    async fn run_command(
        &self,
        Parameters(p): Parameters<RunCommandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::run_command::run_run_command(
            crate::tools::run_command::RunCommandParams {
                command: p.command, timeout_ms: p.timeout_ms,
                working_dir: p.working_dir, env: p.env,
            },
            &self.state.workspace,
        )
        .await
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Run the project test suite. Auto-detects cargo/jest/vitest/pytest.")]
    async fn run_test(
        &self,
        Parameters(p): Parameters<RunTestParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::run_test::run_run_test(
            crate::tools::run_test::RunTestParams {
                filter: p.filter, language: p.language, timeout_ms: p.timeout_ms,
            },
            &self.state.workspace,
        )
        .await
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Run a linter on a file. Auto-detects clippy/eslint/ruff.")]
    async fn lint_file(
        &self,
        Parameters(p): Parameters<LintFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::lint_file::run_lint_file(
            crate::tools::lint_file::LintFileParams {
                path: p.path, linter: p.linter, fix: p.fix,
            },
            &self.state.workspace,
        )
        .await
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Return a structured summary of the current workflow session.")]
    async fn session_summary(
        &self,
        Parameters(_): Parameters<SessionSummaryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::session_summary::run_session_summary(
            &self.state.workflow_dir,
            &self.state.checkpoint,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Return a structured diff of workspace changes relative to latest checkpoint.")]
    async fn diff_since_checkpoint(
        &self,
        Parameters(p): Parameters<DiffSinceCheckpointParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::diff_since_checkpoint::run_diff_since_checkpoint(
            crate::tools::diff_since_checkpoint::DiffSinceCheckpointParams {
                label: p.label, paths: p.paths,
            },
            &self.state.workspace,
            &self.state.checkpoint,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Return a file-level dependency graph. Uses static import analysis.")]
    async fn dependency_graph(
        &self,
        Parameters(p): Parameters<DependencyGraphParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::dependency_graph::run_dependency_graph(
            crate::tools::dependency_graph::DependencyGraphParams {
                path: p.path, depth: p.depth, direction: p.direction,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Preview a symbol rename across the project without writing changes.")]
    async fn symbol_rename_preview(
        &self,
        Parameters(p): Parameters<SymbolRenamePreviewParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::symbol_rename_preview::run_symbol_rename_preview(
            crate::tools::symbol_rename_preview::SymbolRenamePreviewParams {
                path: p.path, line: p.line, column: p.column, new_name: p.new_name,
            },
            &self.state,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Apply edits across multiple files atomically. Validate-then-write in 3 phases.")]
    async fn multi_file_edit(
        &self,
        Parameters(p): Parameters<MultiFileEditParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let cache = self.state.cache.read().await;
        crate::tools::multi_file_edit::run_multi_file_edit(
            crate::tools::multi_file_edit::MultiFileEditParams {
                edits: p.edits.into_iter().map(|fs| crate::tools::multi_file_edit::FileEditSet {
                    file_path: fs.file_path,
                    edits: fs.edits.into_iter().map(|e| crate::tools::multi_file_edit::FileEditItem {
                        old_string: e.old_string, new_string: e.new_string, replace_all: e.replace_all,
                    }).collect(),
                }).collect(),
                verify_syntax: p.verify_syntax,
            },
            &cache, &self.state.checkpoint, &self.state.backup, &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Scaffold batch file/directory creation. Parent dirs auto-created.")]
    async fn create_file_tree(
        &self,
        Parameters(p): Parameters<CreateFileTreeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::create_file_tree::run_create_file_tree(
            crate::tools::create_file_tree::CreateFileTreeParams {
                base_path: p.base_path,
                tree: p.tree.into_iter().map(|n| crate::tools::create_file_tree::FileNode {
                    path: n.path, content: n.content, template: n.template,
                }).collect(),
                overwrite: p.overwrite,
            },
            &self.state.checkpoint, &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Query package registries (crates.io/npm/PyPI) for latest version and metadata.")]
    async fn package_info(
        &self,
        Parameters(p): Parameters<PackageInfoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let params = crate::tools::package_info::PackageInfoParams {
            name: p.name, registry: p.registry, version: p.version,
        };
        match tokio::time::timeout(
            std::time::Duration::from_secs(12),
            crate::tools::package_info::run_package_info(params),
        ).await {
            Ok(Ok(r))  => Ok(to_text(serde_json::to_string_pretty(&r).unwrap())),
            Ok(Err(e)) => Err(to_mcp_err(e)),
            Err(_) => Ok(to_text(serde_json::to_string_pretty(
                &crate::tools::package_info::PackageInfoResult {
                    status: "NETWORK_ERROR".to_string(), registry: "unknown".to_string(),
                    name: String::new(), version: String::new(),
                    description: None, homepage: None, cached: false,
                }
            ).unwrap())),
        }
    }

    #[tool(description = "Run TypeScript (tsc) or Python (mypy) type checking.")]
    async fn type_check(
        &self,
        Parameters(p): Parameters<TypeCheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::type_check::run_type_check(
            crate::tools::type_check::TypeCheckParams { path: p.path, language: p.language },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Read multiple files in a single request (max 20).")]
    async fn batch_read(
        &self,
        Parameters(p): Parameters<BatchReadParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::batch_read::run_batch_read(
            crate::tools::batch_read::BatchReadParams { paths: p.paths },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Format a file in-place using rustfmt/black/prettier/gofmt.")]
    async fn format_file(
        &self,
        Parameters(p): Parameters<FormatFileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::format_file::run_format_file(
            crate::tools::format_file::FormatFileParams { path: p.path, check_only: p.check_only },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Rename/move a file atomically within the workspace.")]
    async fn file_rename(
        &self,
        Parameters(p): Parameters<FileRenameParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::file_rename::run_file_rename(
            crate::tools::file_rename::FileRenameParams { old_path: p.old_path, new_path: p.new_path },
            &self.state.checkpoint,
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Copy the workspace into .snapshots/ for point-in-time recovery.")]
    async fn snapshot_workspace(
        &self,
        Parameters(p): Parameters<SnapshotWorkspaceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::snapshot_workspace::run_snapshot_workspace(
            crate::tools::snapshot_workspace::SnapshotWorkspaceParams {
                label: p.label, include_ignored: p.include_ignored,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Static call-hierarchy analysis: find callers or callees of a symbol.")]
    async fn call_hierarchy(
        &self,
        Parameters(p): Parameters<CallHierarchyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::call_hierarchy::run_call_hierarchy(
            crate::tools::call_hierarchy::CallHierarchyParams {
                symbol: p.symbol, path: p.path, direction: p.direction, limit: p.limit,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Compact journal.jsonl — archive old entries, retain recent N lines.")]
    async fn compact_journal(
        &self,
        Parameters(p): Parameters<CompactJournalParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::compact_journal::run_compact_journal(
            crate::tools::compact_journal::CompactJournalParams {
                max_entries: p.max_entries, summarize: p.summarize.unwrap_or(false),
            },
            &self.state.workflow_dir,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Search journal.jsonl for entries matching query.")]
    async fn search_session(
        &self,
        Parameters(p): Parameters<SearchSessionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::search_session::run_search_session(
            crate::tools::search_session::SearchSessionParams {
                query: p.query, filter_event: p.filter_event, filter_phase: p.filter_phase,
                filter_node: p.filter_node, limit: p.limit, sort_by: p.sort_by,
                since: p.since, until: p.until,
            },
            &self.state.workflow_dir,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_git_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::git_router()
    }
}
