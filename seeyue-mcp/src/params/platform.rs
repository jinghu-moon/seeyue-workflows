// src/params/platform.rs — P2: Windows + tree-sitter tool params

use rmcp::schemars;
use serde::Deserialize;

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
