// src/params/git.rs — P2: Git tool params

use rmcp::schemars;
use serde::Deserialize;

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

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct GitLogParams {
    #[schemars(description = "Max number of commits to return (default: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Filter to commits affecting this file path")]
    pub path:  Option<String>,
    #[schemars(description = "Show commits since this date (e.g. 2024-01-01)")]
    pub since: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitBlameParams {
    #[schemars(description = "File path relative to workspace root")]
    pub path:       String,
    #[schemars(description = "Start line (1-based, default: 1)")]
    pub start_line: Option<usize>,
    #[schemars(description = "End line inclusive (default: EOF)")]
    pub end_line:   Option<usize>,
}
