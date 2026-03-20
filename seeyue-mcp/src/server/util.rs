// src/server/util.rs — Shared conversion helpers for tool handlers
//
// MCP spec (2025-03-26) distinguishes two error kinds:
//   - Protocol errors (ErrorData / JSON-RPC error):   bad params, unknown tool
//   - Tool execution errors (CallToolResult::error):   business/IO failures
//
// `to_mcp_err`  → protocol error  (MissingParameter, PathEscape, InvalidLineRange, InvalidRegex)
// `to_tool_err` → execution error (everything else: file not found, IO, git, LSP, edit failed…)

use rmcp::{
    model::{
        CallToolResult, Content, ErrorData,
        LoggingLevel, LoggingMessageNotificationParam,
        ProgressNotificationParam, ProgressToken,
        ResourceUpdatedNotificationParam,
    },
    service::RequestContext,
    RoleServer,
};
use crate::error::ToolError;

pub fn to_text(s: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(s)])
}

/// Return success result with an audience+priority annotation.
/// `priority` 0.0 = low, 1.0 = high.
pub fn to_text_annotated(s: String, priority: f32) -> CallToolResult {
    use rmcp::model::{Role};
    let content = Content::text(s)
        .with_audience(vec![Role::User, Role::Assistant])
        .with_priority(priority);
    CallToolResult::success(vec![content])
}

/// Convert a ToolError to the appropriate MCP response type.
/// Protocol-level errors (bad parameters) become ErrorData;
/// execution errors become CallToolResult with isError=true.
pub fn tool_error_to_result(e: ToolError) -> Result<CallToolResult, ErrorData> {
    match &e {
        // Parameter/protocol errors → JSON-RPC error
        ToolError::MissingParameter { .. }
        | ToolError::PathEscape { .. }
        | ToolError::InvalidLineRange { .. }
        | ToolError::InvalidRegex { .. } => Err(ErrorData::invalid_params(e.to_json(), None)),
        // All other errors → tool execution error (isError: true)
        _ => Ok(CallToolResult::error(vec![Content::text(e.to_json())])),
    }
}

/// Legacy alias kept for call-sites that expect `Err(ErrorData)`.
/// Only use for genuine protocol errors (invalid params, unknown tool).
pub fn to_mcp_err(e: ToolError) -> ErrorData {
    ErrorData::invalid_params(e.to_json(), None)
}

/// Send a MCP logging notification to the client peer (best-effort, ignore send errors).
/// Only fires if the peer channel is open.
pub fn notify_log(
    ctx:    &RequestContext<RoleServer>,
    level:  LoggingLevel,
    logger: &str,
    data:   serde_json::Value,
) {
    let param = LoggingMessageNotificationParam::new(level, data)
        .with_logger(logger);
    // notify_logging_message returns a future; spawn so caller stays sync-friendly
    let peer = ctx.peer.clone();
    tokio::spawn(async move {
        let _ = peer.notify_logging_message(param).await;
    });
}

/// Send a MCP progress notification.
/// `progress` is the current value (increases monotonically); `total` is optional.
pub fn notify_progress(
    ctx:            &RequestContext<RoleServer>,
    progress_token: ProgressToken,
    progress:       f64,
    total:          Option<f64>,
    message:        Option<String>,
) {
    let param = ProgressNotificationParam {
        progress_token,
        progress,
        total,
        message,
    };
    let peer = ctx.peer.clone();
    tokio::spawn(async move {
        let _ = peer.notify_progress(param).await;
    });
}

/// Notify the client that a resource URI has been updated.
pub fn notify_resource_updated(ctx: &RequestContext<RoleServer>, uri: impl Into<String>) {
    let param = ResourceUpdatedNotificationParam { uri: uri.into() };
    let peer = ctx.peer.clone();
    tokio::spawn(async move {
        let _ = peer.notify_resource_updated(param).await;
    });
}
