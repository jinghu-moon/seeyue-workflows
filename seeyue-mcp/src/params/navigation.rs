// src/params/navigation.rs — P2: Search & Navigation tool params

use rmcp::schemars;
use serde::Deserialize;

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
