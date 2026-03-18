// src/server/tools_mem.rs — Memory / Checkpoint / TDD / Session-End Tools

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*};
use crate::params::*;
use crate::server::util::{to_text, to_mcp_err};
use super::SeeyueMcpServer;

#[tool_router(router = mem_router)]
impl SeeyueMcpServer {
    #[tool(description = "Persist a named memory entry to .ai/memory/<key>.md. Key: alphanumeric/dash/underscore/slash. Updates index.json for fast lookup.")]
    async fn memory_write(
        &self,
        Parameters(p): Parameters<MemoryWriteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::memory_write::run_memory_write(
            crate::tools::memory_write::MemoryWriteParams {
                key:     p.key,
                content: p.content,
                tags:    p.tags,
                mode:    p.mode,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Search persisted memory entries from .ai/memory/. Matches query against key/tags/content. Returns full content when exactly one entry matches.")]
    async fn memory_read(
        &self,
        Parameters(p): Parameters<MemoryReadParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::memory_read::run_memory_read(
            crate::tools::memory_read::MemoryReadParams {
                query: p.query,
                tag:   p.tag,
                limit: p.limit,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Delete a named memory entry from .ai/memory/. Removes content file and index.json entry. Returns not_found if key does not exist.")]
    async fn memory_delete(
        &self,
        Parameters(p): Parameters<MemoryDeleteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::memory_delete::run_memory_delete(
            crate::tools::memory_delete::MemoryDeleteParams { key: p.key },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "List all persisted memory entries from .ai/memory/. Supports optional tag filter and limit. Sorted by updated timestamp descending.")]
    async fn memory_list(
        &self,
        Parameters(p): Parameters<MemoryListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::memory_list::run_memory_list(
            crate::tools::memory_list::MemoryListParams {
                tag:   p.tag,
                limit: p.limit,
            },
            &self.state.workspace,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "List all checkpoint snapshots in the current session. Returns file path, tool name, and captured_at timestamp. Use with rewind to undo.")]
    async fn checkpoint_list(
        &self,
        Parameters(_): Parameters<CheckpointListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::checkpoint_list::run_checkpoint_list(
            crate::tools::checkpoint_list::CheckpointListParams {},
            &self.state.checkpoint,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "Aggregate TDD evidence from journal.jsonl. Returns per-node TDD progression: red_verified, green_verified, refactor_done. Optional node_id filter.")]
    async fn tdd_evidence_summary(
        &self,
        Parameters(p): Parameters<TddEvidenceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        crate::tools::tdd_evidence::run_tdd_evidence(
            crate::tools::tdd_evidence::TddEvidenceParams { node_id: p.node_id },
            &self.state.workflow_dir,
        )
        .map(|r| to_text(serde_json::to_string_pretty(&r).unwrap()))
        .map_err(to_mcp_err)
    }

    #[tool(description = "[Hook] End session and persist a session summary to .ai/memory/sessions/. Extracts nodes visited and files written from journal. Returns memory_key.")]
    async fn sy_session_end(
        &self,
        Parameters(p): Parameters<SessionEndParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::hooks::run_session_end(
            crate::tools::hooks::SessionEndParams { note: p.note },
            &self.state,
        );
        Ok(to_text(serde_json::to_string_pretty(&result).unwrap()))
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_mem_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::mem_router()
    }
}
