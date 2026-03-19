// src/tools/dispatch.rs
// Unified dispatch layer: route tool name to handler.
// Phase M-N3: defines routing table and error types.
// Does not replace rmcp macro routing; provides a typed dispatch abstraction.

/// Dispatch errors corresponding to MCP error codes.
#[derive(Debug, Clone)]
pub enum DispatchError {
    /// Tool name not found in route table (-32601)
    MethodNotFound(String),
    /// Parameter parsing failed (-32602)
    InvalidParams(String),
    /// Tool requires workspace but none configured
    WorkspaceRequired,
    /// Tool is registered but not active in current session
    ToolDisabled(String),
}

impl DispatchError {
    pub fn method_not_found(name: &str) -> Self {
        DispatchError::MethodNotFound(format!("Tool '{}' not found", name))
    }
}

/// Check whether a tool name has a known route.
/// Returns true for all tools registered in the metadata registry.
pub fn route_exists(name: &str) -> bool {
    crate::tools::metadata::registry().contains_key(name)
}
