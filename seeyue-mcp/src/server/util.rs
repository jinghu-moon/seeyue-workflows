// src/server/util.rs — Shared conversion helpers for tool handlers

use rmcp::model::{CallToolResult, Content, ErrorData};
use crate::error::ToolError;

pub fn to_text(s: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(s)])
}

pub fn to_mcp_err(e: ToolError) -> ErrorData {
    ErrorData::invalid_params(e.to_json(), None)
}
