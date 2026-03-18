// src/server/tools_nav.rs — P2 Windows/tree-sitter/Search/LSP Navigation Tools

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*};
use crate::params::*;
use crate::server::util::{to_text, to_mcp_err};
use super::SeeyueMcpServer;

#[tool_router(router = nav_router)]
impl SeeyueMcpServer {
    #[tool(description = "\
        Normalize and resolve a Windows path relative to the workspace. \
        Returns absolute + relative paths, existence, and directory status. \
        Rejects path escape outside workspace.")]
    async fn resolve_path(
        &self,
        Parameters(p): Parameters<ResolvePathParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::resolve_path::run_resolve_path(
            crate::tools::resolve_path::ResolvePathParams { path: p.path },
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
        let result = crate::tools::env_info::run_env_info(&self.state.workspace);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        Return a compact symbol outline of a source file using tree-sitter. \
        Designed to be ~200 tokens and used with read_range for focused reads.")]
    async fn file_outline(
        &self,
        Parameters(p): Parameters<FileOutlineParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::file_outline::run_file_outline(
            crate::tools::file_outline::FileOutlineParams {
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
        crate::tools::verify_syntax::run_verify_syntax(
            crate::tools::verify_syntax::VerifySyntaxParams {
                path:     p.path,
                content:  p.content,
                language: p.language,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "\
        Read a specific line range, optionally resolved from a symbol name. \
        Supports context lines around the target range.")]
    async fn read_range(
        &self,
        Parameters(p): Parameters<ReadRangeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::read_range::run_read_range(
            crate::tools::read_range::ReadRangeParams {
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
        crate::tools::search_workspace::run_search_workspace(
            crate::tools::search_workspace::SearchWorkspaceParams {
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
        crate::tools::workspace_tree::run_workspace_tree(
            crate::tools::workspace_tree::WorkspaceTreeParams {
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

    #[tool(description = "\
        Read a file with progressive compression to fit a token budget. \
        Applies up to 4 compression levels (blank lines → comments → imports → skeleton).")]
    async fn read_compressed(
        &self,
        Parameters(p): Parameters<ReadCompressedParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::read_compressed::run_read_compressed(
            crate::tools::read_compressed::ReadCompressedParams {
                path:         p.path,
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
        crate::tools::preview_edit::run_preview_edit(
            crate::tools::preview_edit::PreviewEditParams {
                file_path:   p.file_path,
                old_string:  p.old_string,
                new_string:  p.new_string,
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
        crate::tools::find_definition::run_find_definition(
            crate::tools::find_definition::FindDefinitionParams {
                path:   p.path,
                line:   p.line,
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
        crate::tools::find_references::run_find_references(
            crate::tools::find_references::FindReferencesParams {
                path:   p.path,
                line:   p.line,
                column: p.column,
            },
            &self.state,
        )
        .await
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_nav_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::nav_router()
    }
}
