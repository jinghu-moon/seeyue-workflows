// src/params/editing.rs — P0: File editing tool params

use rmcp::schemars;
use serde::Deserialize;

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
