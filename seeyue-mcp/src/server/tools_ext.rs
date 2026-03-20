// src/server/tools_ext.rs — Extended Tools (find_files, hover, on_error, editor, process, script, budget)

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*};
use crate::params::*;
use crate::server::util::{to_text, tool_error_to_result};
use super::SeeyueMcpServer;

#[tool_router(router = ext_router)]
impl SeeyueMcpServer {
    #[tool(description = "Find files matching a glob pattern. Respects .gitignore by default.")]
    async fn find_files(
        &self,
        Parameters(p): Parameters<FindFilesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::find_files::run_find_files(
            crate::tools::find_files::FindFilesParams {
                pattern:           p.pattern,
                respect_gitignore: p.respect_gitignore,
                show_hidden:       p.show_hidden,
                limit:             p.limit,
            },
            &self.state.workspace,
        )
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Get LSP hover info (type signature, docs) for symbol at given position.")]
    async fn get_hover_info(
        &self,
        Parameters(p): Parameters<GetHoverInfoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::get_hover_info::run_get_hover_info(
            crate::tools::get_hover_info::GetHoverInfoParams {
                path:   p.path,
                line:   p.line as usize,
                column: p.column as usize,
            },
            &self.state,
        )
        .await
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Record a tool error to journal, optionally notify, return recovery suggestions.")]
    async fn sy_on_error(
        &self,
        Parameters(p): Parameters<OnErrorParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::on_error::run_on_error(
            crate::tools::on_error::OnErrorParams {
                tool:       p.tool,
                error:      p.error,
                error_kind: p.error_kind,
                path:       p.path,
                notify:     p.notify,
                node_id:    p.node_id,
                run_id:     p.run_id,
            },
            &self.state.workflow_dir,
        )
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Open a file in VS Code or Cursor at a given line/column. editor: auto|vscode|cursor.")]
    async fn open_in_editor(
        &self,
        Parameters(p): Parameters<OpenInEditorParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::open_in_editor::run_open_in_editor(
            crate::tools::open_in_editor::OpenInEditorParams {
                path:   p.path,
                line:   p.line,
                column: p.column,
                editor: p.editor,
            },
            &self.state.workspace,
        )
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "List running processes (Windows tasklist). Filter by name substring or port.")]
    async fn process_list(
        &self,
        Parameters(p): Parameters<ProcessListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::process_list::run_process_list(
            crate::tools::process_list::ProcessListParams {
                filter_name: p.filter_name,
                filter_port: p.filter_port,
                limit:       p.limit,
            },
        )
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Run a script file. Supports .ps1 .sh .py .js .ts. Configurable timeout.")]
    async fn run_script(
        &self,
        Parameters(p): Parameters<RunScriptParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::run_script::run_script(
            crate::tools::run_script::RunScriptParams {
                script:       p.script,
                args:         p.args,
                working_dir:  p.working_dir,
                timeout_secs: p.timeout_secs,
                env:          p.env,
            },
            &self.state,
        )
        .await
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Check loop budget consumption. Warns (Toast + journal) when warn_at fraction exceeded.")]
    async fn session_budget_warning(
        &self,
        Parameters(p): Parameters<SessionBudgetWarningParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::session_budget_warning::run_session_budget_warning(
            crate::tools::session_budget_warning::SessionBudgetWarningParams {
                threshold: p.warn_at.map(|v| v as f32),
                notify:    p.notify,
            },
            &self.state.workflow_dir,
        )
        .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_ext_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::ext_router()
    }
}
