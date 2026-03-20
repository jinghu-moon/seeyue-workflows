// src/server/tools_nav.rs — P2 Windows/tree-sitter/Search/LSP Navigation Tools

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*};
use crate::params::*;
use crate::server::util::{to_text, tool_error_to_result};
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        Call FIRST before reading file body — ~200 tokens vs full file. \
        Use with read_range for focused reads of specific symbols.")]
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "\
        Search the workspace for a pattern. \
        PREFER over Grep/Glob for workspace searches — gitignore-aware and supports regex + literal. \
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
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
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "\
        Find symbols by name or name_path pattern across the workspace. \
        PREFER over Grep for any symbol/function/class/struct search — \
        semantic, index-accelerated (up to 7× faster on large codebases). \
        Supports exact match or substring. Optionally include source body. \
        Use relative_path to restrict to a single file.")]
    async fn find_symbol(
        &self,
        Parameters(p): Parameters<FindSymbolParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::find_symbol::run_find_symbol(
            crate::tools::find_symbol::FindSymbolParams {
                name_path_pattern:  p.name_path_pattern,
                relative_path:      p.relative_path,
                substring_matching: p.substring_matching,
                include_body:       p.include_body,
                depth:              p.depth,
            },
            &self.state,
        )
        .await
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "\
        Replace the complete body of a symbol (function, method, struct, etc.) in-place. \
        PREFER over edit when replacing an entire symbol definition — \
        finds the symbol by name_path, replaces start_line..end_line atomically. \
        Requires relative_path to avoid ambiguity across files.")]
    async fn replace_symbol_body(
        &self,
        Parameters(p): Parameters<ReplaceSymbolBodyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::replace_symbol_body::run_replace_symbol_body(
            crate::tools::replace_symbol_body::ReplaceSymbolBodyParams {
                name_path:     p.name_path,
                relative_path: p.relative_path,
                new_body:      p.new_body,
            },
            &self.state,
        )
        .await
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "\
        Insert content immediately after a symbol's closing line. \
        Use to add a new function/method/field after an existing symbol. \
        Requires relative_path. Uses atomic write.")]
    async fn insert_after_symbol(
        &self,
        Parameters(p): Parameters<InsertAfterSymbolParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::insert_symbol::run_insert_after_symbol(
            crate::tools::insert_symbol::InsertSymbolParams {
                name_path:     p.name_path,
                relative_path: p.relative_path,
                content:       p.content,
            },
            &self.state,
        )
        .await
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "\
        Insert content immediately before a symbol's opening line. \
        Use to add imports, attributes, or new symbols before an existing one. \
        Requires relative_path. Uses atomic write.")]
    async fn insert_before_symbol(
        &self,
        Parameters(p): Parameters<InsertBeforeSymbolParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::insert_symbol::run_insert_before_symbol(
            crate::tools::insert_symbol::InsertSymbolParams {
                name_path:     p.name_path,
                relative_path: p.relative_path,
                content:       p.content,
            },
            &self.state,
        )
        .await
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_nav_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::nav_router()
    }
}
