// src/params.rs
//
// MCP tool parameter structs for SeeyueMcpServer.
// Extracted from main.rs to keep it focused on routing logic.
//
// All structs derive Debug + Deserialize + schemars::JsonSchema
// so they can be used directly with rmcp Parameters<T> extractor.

use rmcp::schemars;
use serde::Deserialize;

// ─── P0: File editing ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadFileParams {
    #[schemars(description = "File path relative to workspace root (forward or back slashes both ok)")]
    pub file_path:  String,
    #[schemars(description = "Start line, 1-based (default: 1)")]
    pub start_line: Option<u32>,
    #[schemars(description = "End line inclusive (default: EOF). Max 2000 lines per call.")]
    pub end_line:   Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteParams {
    #[schemars(description = "File path relative to workspace root")]
    pub file_path: String,
    #[schemars(description = "Complete file content. Encoding and line endings are preserved on overwrite.")]
    pub content:   String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditParams {
    #[schemars(description = "File path relative to workspace root")]
    pub file_path:  String,
    #[schemars(description = "Exact string to replace. Copy verbatim from read_file output — tabs are \\t, not spaces.")]
    pub old_string: String,
    #[schemars(description = "Replacement string. Empty string = delete old_string.")]
    pub new_string: String,
    #[schemars(description = "Replace all occurrences (default: false — fail if multiple matches)")]
    pub replace_all: Option<bool>,
    #[schemars(description = "Skip cache freshness check (default: false)")]
    pub force:      Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SingleEdit {
    pub old_string:  String,
    pub new_string:  String,
    pub replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiEditParams {
    #[schemars(description = "File path relative to workspace root")]
    pub file_path: String,
    #[schemars(description = "Ordered list of edits to apply atomically")]
    pub edits:     Vec<SingleEdit>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RewindParams {
    #[schemars(description = "Number of write operations to undo (default: 1)")]
    pub steps: Option<u32>,
}

// ─── P2: Windows + tree-sitter ───────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ResolvePathParams {
    #[schemars(description = "Any path form (forward/back slashes, .., ~). Returned as normalized absolute path.")]
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct EnvInfoParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileOutlineParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:  String,
    #[schemars(description = "Outline depth: 0=top-level, 1=include methods (default), 2=all descendants")]
    pub depth: Option<u8>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VerifySyntaxParams {
    #[schemars(description = "File path relative to workspace root (optional if content is provided)")]
    pub path:     Option<String>,
    #[schemars(description = "Source content to verify (optional if path is provided)")]
    pub content:  Option<String>,
    #[schemars(description = "Language hint when content is provided (rust/python/typescript/tsx/go)")]
    pub language: Option<String>,
}

// ─── P2: Search & Navigation ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadRangeParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:          String,
    #[schemars(description = "Start line (1-based)")]
    pub start:         Option<usize>,
    #[schemars(description = "End line (1-based)")]
    pub end:           Option<usize>,
    #[schemars(description = "Symbol name to resolve range from file_outline")]
    pub symbol:        Option<String>,
    #[schemars(description = "Context lines to include above and below")]
    pub context_lines: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchWorkspaceParams {
    #[schemars(description = "Search pattern (regex or literal)")]
    pub pattern:       String,
    #[schemars(description = "Whether pattern is a regex (default: false)")]
    pub is_regex:      Option<bool>,
    #[schemars(description = "Optional file glob filter (e.g., src/**/*.rs)")]
    pub file_glob:     Option<String>,
    #[schemars(description = "Context lines to include above and below")]
    pub context_lines: Option<usize>,
    #[schemars(description = "Max results to return (default: 50)")]
    pub max_results:   Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorkspaceTreeParams {
    #[schemars(description = "Max directory depth (default: 3)")]
    pub depth:             Option<usize>,
    #[schemars(description = "Respect .gitignore/.ignore (default: true)")]
    pub respect_gitignore: Option<bool>,
    #[schemars(description = "Show hidden files (default: false)")]
    pub show_hidden:       Option<bool>,
    #[schemars(description = "Minimum file size in bytes to include (default: 0)")]
    pub min_size_bytes:    Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadCompressedParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:         String,
    #[schemars(description = "Target token budget (default: 800)")]
    pub token_budget: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreviewEditParams {
    #[schemars(description = "File path relative to workspace root")]
    pub file_path:  String,
    #[schemars(description = "Exact string to replace")]
    pub old_string: String,
    #[schemars(description = "Replacement string")]
    pub new_string: String,
    #[schemars(description = "Replace all occurrences (default: false)")]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindDefinitionParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:   String,
    #[schemars(description = "1-based line number")]
    pub line:   usize,
    #[schemars(description = "1-based column number")]
    pub column: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindReferencesParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:   String,
    #[schemars(description = "1-based line number")]
    pub line:   usize,
    #[schemars(description = "1-based column number")]
    pub column: usize,
}

// ─── P2: Git ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitStatusParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitDiffFileParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:   String,
    #[schemars(description = "Base git ref (default: HEAD)")]
    pub base:   Option<String>,
    #[schemars(description = "Use staged version instead of working tree (default: false)")]
    pub staged: Option<bool>,
}

// ─── P3: Execution ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SessionStartParams {
    #[serde(default)]
    pub skip_recovery: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunCommandParams {
    #[schemars(description = "Shell command to execute")]
    pub command: String,
    #[schemars(description = "Timeout in milliseconds (default: 30000, max: 300000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Working directory relative to workspace (default: workspace root)")]
    pub working_dir: Option<String>,
    #[schemars(description = "Extra environment variables to inject")]
    pub env: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunTestParams {
    #[schemars(description = "Test name filter (optional)")]
    pub filter: Option<String>,
    #[schemars(description = "Language hint: rust|jest|vitest|typescript|python (default: auto-detect)")]
    pub language: Option<String>,
    #[schemars(description = "Timeout in milliseconds (default: 60000, max: 300000)")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LintFileParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path: String,
    #[schemars(description = "Linter override: clippy|eslint|ruff (default: auto-detect from extension)")]
    pub linter: Option<String>,
    #[schemars(description = "Apply auto-fix where possible (default: false)")]
    pub fix: Option<bool>,
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
    pub path: String,
    #[schemars(description = "Traversal depth (default: 2, max: 5)")]
    pub depth: Option<usize>,
    #[schemars(description = "Direction: imports | imported_by | both (default: imports)")]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SymbolRenamePreviewParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path: String,
    #[schemars(description = "1-based line number of the symbol")]
    pub line: usize,
    #[schemars(description = "1-based column number of the symbol")]
    pub column: usize,
    #[schemars(description = "New name to preview the rename with")]
    pub new_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiFileEditItem {
    #[schemars(description = "String to find (exact match)")]
    pub old_string: String,
    #[schemars(description = "Replacement string")]
    pub new_string: String,
    #[schemars(description = "Replace all occurrences (default false)")]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiFileEditSet {
    #[schemars(description = "File path relative to workspace root")]
    pub file_path: String,
    #[schemars(description = "List of edits to apply to this file")]
    pub edits: Vec<MultiFileEditItem>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MultiFileEditParams {
    #[schemars(description = "List of file edit sets (max 20 files)")]
    pub edits: Vec<MultiFileEditSet>,
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
    pub tree: Vec<FileNode>,
    #[schemars(description = "Overwrite existing files (default false = skip)")]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PackageInfoParams {
    #[schemars(description = "Package name (e.g. serde, react, numpy)")]
    pub name: String,
    #[schemars(description = "Registry: crates | npm | pypi (auto-detected if omitted)")]
    pub registry: Option<String>,
    #[schemars(description = "Specific version to query (default: latest)")]
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TypeCheckParams {
    #[schemars(description = "File or directory path relative to workspace root")]
    pub path: String,
    #[schemars(description = "Language: typescript | python (auto-detected if omitted)")]
    pub language: Option<String>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitLogParams {
    #[schemars(description = "Max commits to return (default: 20, max: 200)")]
    pub limit: Option<usize>,
    #[schemars(description = "Restrict to commits touching this relative path")]
    pub path: Option<String>,
    #[schemars(description = "Starting ref/branch/tag (default: HEAD)")]
    pub since: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BatchReadParams {
    #[schemars(description = "List of relative file paths to read (max 20)")]
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FormatFileParams {
    #[schemars(description = "Relative file path to format")]
    pub path: String,
    #[schemars(description = "If true, only check formatting without writing (default: false)")]
    pub check_only: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitBlameParams {
    #[schemars(description = "Relative file path")]
    pub path: String,
    #[schemars(description = "Start line (1-based, optional)")]
    pub start_line: Option<usize>,
    #[schemars(description = "End line (1-based, inclusive, optional)")]
    pub end_line: Option<usize>,
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
    pub label: Option<String>,
    #[schemars(description = "If true, also copy .gitignore'd files (default: false)")]
    pub include_ignored: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CallHierarchyParams {
    #[schemars(description = "Symbol name to analyse (function/method)")]
    pub symbol: String,
    #[schemars(description = "callers | callees | both (default: callers)")]
    pub direction: Option<String>,
    #[schemars(description = "Max results (default: 50, max: 200)")]
    pub limit: Option<usize>,
    #[schemars(description = "Restrict search to this relative sub-path (optional)")]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CompactJournalParams {
    #[schemars(description = "Maximum recent lines to retain (default: 200)")]
    pub max_entries: Option<usize>,
    #[schemars(description = "Append event-type summary to session.yaml notes (default: false)")]
    pub summarize: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSessionParams {
    #[schemars(description = "Free-text query matched against journal entries")]
    pub query: String,
    #[schemars(description = "Filter by exact event type (e.g. write_recorded)")]
    pub filter_event: Option<String>,
    #[schemars(description = "Filter by phase id/name")]
    pub filter_phase: Option<String>,
    #[schemars(description = "Filter by node id/name")]
    pub filter_node: Option<String>,
    #[schemars(description = "Maximum results to return (default: 20, max: 200)")]
    pub limit: Option<usize>,
    #[schemars(description = "Sort order: timestamp (default) | event_weight (important events first)")]
    pub sort_by: Option<String>,
}

// ─── Memory Tools ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryWriteParams {
    #[schemars(description = "Memory key (alphanumeric, dash, underscore, slash). E.g. decisions/arch-v4")]
    pub key: String,
    #[schemars(description = "Markdown content to store")]
    pub content: String,
    #[serde(default)]
    #[schemars(description = "Optional tags for retrieval")]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryReadParams {
    #[schemars(description = "Free-text query matched against key, tags, and content preview")]
    pub query: String,
    #[schemars(description = "Filter by tag (exact match on any tag in the entry)")]
    pub tag: Option<String>,
    #[schemars(description = "Maximum entries to return (default: 10, max: 50)")]
    pub limit: Option<usize>,
}
