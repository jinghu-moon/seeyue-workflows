// src/params/extended.rs — New tools: find_files, get_hover_info, on_error,
// open_in_editor, process_list, run_script, session_budget_warning

use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindFilesParams {
    #[schemars(description = "Glob pattern to match (e.g. **/*.rs, src/**/*.ts)")]
    pub pattern:           String,
    #[schemars(description = "Respect .gitignore rules (default: true)")]
    pub respect_gitignore: Option<bool>,
    #[schemars(description = "Include hidden files/directories (default: false)")]
    pub show_hidden:       Option<bool>,
    #[schemars(description = "Maximum results to return (default: 100, max: 500)")]
    pub limit:             Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHoverInfoParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:   String,
    #[schemars(description = "1-based line number")]
    pub line:   u32,
    #[schemars(description = "1-based column number")]
    pub column: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct OnErrorParams {
    #[schemars(description = "Tool name that failed (e.g. edit, run_command)")]
    pub tool:       String,
    #[schemars(description = "Error message or structured error JSON")]
    pub error:      String,
    #[schemars(description = "Error kind hint: io | syntax | lsp | policy | timeout | unknown")]
    pub error_kind: Option<String>,
    #[schemars(description = "File path involved (if any)")]
    pub path:       Option<String>,
    #[schemars(description = "Send a Windows Toast notification (default: false)")]
    pub notify:     Option<bool>,
    #[schemars(description = "Node id context")]
    pub node_id:    Option<String>,
    #[schemars(description = "Run id context")]
    pub run_id:     Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct OpenInEditorParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:   String,
    #[schemars(description = "1-based line number to jump to (optional)")]
    pub line:   Option<usize>,
    #[schemars(description = "1-based column (optional)")]
    pub column: Option<usize>,
    #[schemars(description = "Editor: vscode | cursor | auto (default: auto, tries cursor first)")]
    pub editor: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct ProcessListParams {
    #[schemars(description = "Filter by process name substring (case-insensitive)")]
    pub filter_name: Option<String>,
    #[schemars(description = "Filter by port number (checks netstat, Windows only)")]
    pub filter_port: Option<u16>,
    #[schemars(description = "Maximum results to return (default: 50, max: 200)")]
    pub limit:       Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunScriptParams {
    #[schemars(description = "Script file path relative to workspace root (.ps1/.sh/.py/.js/.ts)")]
    pub script:       String,
    #[schemars(description = "Arguments to pass to the script")]
    pub args:         Option<Vec<String>>,
    #[schemars(description = "Working directory relative to workspace root (default: workspace root)")]
    pub working_dir:  Option<String>,
    #[schemars(description = "Timeout in seconds (default: 30, max: 300)")]
    pub timeout_secs: Option<u64>,
    #[schemars(description = "Environment variables to set")]
    pub env:          Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SessionBudgetWarningParams {
    #[schemars(description = "Warning threshold as fraction of budget (default: 0.8)")]
    pub warn_at: Option<f64>,
    #[schemars(description = "Send a Toast notification when threshold crossed (default: true)")]
    pub notify:  Option<bool>,
}
