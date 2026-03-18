// src/params/execution.rs — P3: Execution, analysis, extended tool params

use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SessionStartParams {
    #[serde(default)]
    pub skip_recovery: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunCommandParams {
    #[schemars(description = "Shell command to execute")]
    pub command:     String,
    #[schemars(description = "Timeout in milliseconds (default: 30000, max: 300000)")]
    pub timeout_ms:  Option<u64>,
    #[schemars(description = "Working directory relative to workspace (default: workspace root)")]
    pub working_dir: Option<String>,
    #[schemars(description = "Extra environment variables to inject")]
    pub env:         Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunTestParams {
    #[schemars(description = "Test name filter (optional)")]
    pub filter:     Option<String>,
    #[schemars(description = "Language hint: rust|jest|vitest|typescript|python (default: auto-detect)")]
    pub language:   Option<String>,
    #[schemars(description = "Timeout in milliseconds (default: 60000, max: 300000)")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LintFileParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:   String,
    #[schemars(description = "Linter override: clippy|eslint|ruff (default: auto-detect from extension)")]
    pub linter: Option<String>,
    #[schemars(description = "Apply auto-fix where possible (default: false)")]
    pub fix:    Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SessionSummaryParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DiffSinceCheckpointParams {
    #[schemars(description = "Checkpoint label to diff against (default: most recent)")]
    pub label: Option<String>,
    #[schemars(description = "Filter to specific file paths (substring match)")]
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DependencyGraphParams {
    #[schemars(description = "File path relative to workspace root (starting point)")]
    pub path:      String,
    #[schemars(description = "Traversal depth (default: 2, max: 5)")]
    pub depth:     Option<usize>,
    #[schemars(description = "Direction: imports | imported_by | both (default: imports)")]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SymbolRenamePreviewParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:     String,
    #[schemars(description = "1-based line number of the symbol")]
    pub line:     usize,
    #[schemars(description = "1-based column number of the symbol")]
    pub column:   usize,
    #[schemars(description = "New name to preview the rename with")]
    pub new_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiFileEditItem {
    #[schemars(description = "String to find (exact match)")]
    pub old_string:  String,
    #[schemars(description = "Replacement string")]
    pub new_string:  String,
    #[schemars(description = "Replace all occurrences (default false)")]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiFileEditSet {
    #[schemars(description = "File path relative to workspace root")]
    pub file_path: String,
    #[schemars(description = "List of edits to apply to this file")]
    pub edits:     Vec<MultiFileEditItem>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiFileEditParams {
    #[schemars(description = "List of file edit sets (max 20 files)")]
    pub edits:         Vec<MultiFileEditSet>,
    #[schemars(description = "Run tree-sitter syntax check after edits (default true)")]
    pub verify_syntax: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileNode {
    #[schemars(description = "File or directory path relative to base_path. Trailing slash = directory.")]
    pub path:     String,
    #[schemars(description = "File content (omit for directories)")]
    pub content:  Option<String>,
    #[schemars(description = "Template name to use for content generation")]
    pub template: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateFileTreeParams {
    #[schemars(description = "Base directory relative to workspace root")]
    pub base_path: String,
    #[schemars(description = "List of file/directory nodes to create")]
    pub tree:      Vec<FileNode>,
    #[schemars(description = "Overwrite existing files (default false = skip)")]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PackageInfoParams {
    #[schemars(description = "Package name (e.g. serde, react, numpy)")]
    pub name:     String,
    #[schemars(description = "Registry: crates | npm | pypi (auto-detected if omitted)")]
    pub registry: Option<String>,
    #[schemars(description = "Specific version to query (default: latest)")]
    pub version:  Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TypeCheckParams {
    #[schemars(description = "File or directory path relative to workspace root")]
    pub path:     String,
    #[schemars(description = "Language: typescript | python (auto-detected if omitted)")]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BatchReadParams {
    #[schemars(description = "List of relative file paths to read (max 20)")]
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FormatFileParams {
    #[schemars(description = "Relative file path to format")]
    pub path:       String,
    #[schemars(description = "If true, only check formatting without writing (default: false)")]
    pub check_only: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileRenameParams {
    #[schemars(description = "Source relative path")]
    pub old_path: String,
    #[schemars(description = "Destination relative path")]
    pub new_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SnapshotWorkspaceParams {
    #[schemars(description = "Optional label for the snapshot directory")]
    pub label:           Option<String>,
    #[schemars(description = "If true, also copy .gitignore'd files (default: false)")]
    pub include_ignored: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CallHierarchyParams {
    #[schemars(description = "Symbol name to analyse (function/method)")]
    pub symbol:    String,
    #[schemars(description = "callers | callees | both (default: callers)")]
    pub direction: Option<String>,
    #[schemars(description = "Max results (default: 50, max: 200)")]
    pub limit:     Option<usize>,
    #[schemars(description = "Restrict search to this relative sub-path (optional)")]
    pub path:      Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CompactJournalParams {
    #[schemars(description = "Maximum recent lines to retain (default: 200)")]
    pub max_entries: Option<usize>,
    #[schemars(description = "Append event-type summary to session.yaml notes (default: false)")]
    pub summarize:   Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSessionParams {
    #[schemars(description = "Free-text query matched against journal entries")]
    pub query:        String,
    #[schemars(description = "Filter by exact event type (e.g. write_recorded)")]
    pub filter_event: Option<String>,
    #[schemars(description = "Filter by phase id/name")]
    pub filter_phase: Option<String>,
    #[schemars(description = "Filter by node id/name")]
    pub filter_node:  Option<String>,
    #[schemars(description = "Maximum results to return (default: 20, max: 200)")]
    pub limit:        Option<usize>,
    #[schemars(description = "Sort order: timestamp (default) | event_weight (important events first)")]
    pub sort_by:      Option<String>,
    #[schemars(description = "Include only events at or after this ISO 8601 timestamp")]
    pub since:        Option<String>,
    #[schemars(description = "Include only events at or before this ISO 8601 timestamp")]
    pub until:        Option<String>,
}
