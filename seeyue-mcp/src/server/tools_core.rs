// src/server/tools_core.rs — P0 File Editing + P1 Hook Tools

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*};
use crate::params::*;
use crate::server::util::{to_text, to_mcp_err};
use super::SeeyueMcpServer;

#[tool_router(router = core_router)]
impl SeeyueMcpServer {
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
        crate::tools::read::run_read(
            crate::tools::read::ReadParams {
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
        crate::tools::write::run_write(
            crate::tools::write::WriteParams {
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
        crate::tools::edit::run_edit(
            crate::tools::edit::EditParams {
                file_path:   p.file_path,
                old_string:  p.old_string,
                new_string:  p.new_string,
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
        let edits = p.edits.into_iter().map(|e| crate::tools::edit::SingleEdit {
            old_string:  e.old_string,
            new_string:  e.new_string,
            replace_all: e.replace_all.unwrap_or(false),
            expected_replacements: None,
        }).collect();
        crate::tools::edit::run_multi_edit(
            crate::tools::edit::MultiEditParams {
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

    // ── P1 Hook Tools ────────────────────────────────────────────────────────

    #[tool(description = "\
        [Hook] Pre-tool check for Bash commands. \
        Classifies command (destructive/privileged/git_mutating/network_sensitive/etc), \
        checks approval matrix, loop budget, and git special rules. \
        Returns verdict: allow, block, or block_with_approval_request.")]
    async fn sy_pretool_bash(
        &self,
        Parameters(p): Parameters<crate::tools::hooks::PreToolBashParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_pretool_bash(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Pre-tool check for Write/Edit operations. \
        Classifies file (secret_material/security_boundary/system_file/etc), \
        checks approval matrix, TDD state, and scope drift. \
        Returns verdict: allow, block, or block_with_approval_request.")]
    async fn sy_pretool_write(
        &self,
        Parameters(p): Parameters<crate::tools::hooks::PreToolWriteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_pretool_write(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Post-tool evidence capture for Write/Edit operations. \
        Records write event to journal.jsonl for audit trail. \
        Always returns allow.")]
    async fn sy_posttool_write(
        &self,
        Parameters(p): Parameters<crate::tools::hooks::PostToolWriteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_posttool_write(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Stop gate check. \
        Verifies loop budget, pending approvals, and restore state. \
        Returns allow or force_continue (prevent premature stop).")]
    async fn sy_stop(
        &self,
        Parameters(p): Parameters<crate::tools::hooks::StopParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_stop(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        Create a named checkpoint with optional file snapshots. \
        Records event to journal. Snapshots can be restored with rewind.")]
    async fn sy_create_checkpoint(
        &self,
        Parameters(p): Parameters<crate::tools::hooks::CreateCheckpointParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_create_checkpoint(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        Advance workflow to a new node. \
        Updates session.yaml with new node info (id, name, status, TDD state, targets). \
        Records node_exited and node_entered events to journal.")]
    async fn sy_advance_node(
        &self,
        Parameters(p): Parameters<crate::tools::hooks::AdvanceNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_advance_node(p, &self.state);
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }

    #[tool(description = "\
        [Hook] Bootstrap session and run crash-recovery journal replay. \
        Scans journal.jsonl for orphan tool_request events (request with no completion), \
        appends aborted events, and determines safe TDD resume point. \
        Returns session summary including run_id, phase, tdd_state, and recovery_status.")]
    async fn sy_session_start(
        &self,
        Parameters(p): Parameters<SessionStartParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_session_start(
            crate::tools::hooks::SessionStartParams {
                skip_recovery: p.skip_recovery,
            },
            &self.state,
        );
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_core_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::core_router()
    }
}
