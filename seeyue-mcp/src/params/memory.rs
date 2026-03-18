// src/params/memory.rs — Memory & checkpoint tool params

use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryWriteParams {
    #[schemars(description = "Memory key (alphanumeric, dash, underscore, slash). E.g. decisions/arch-v4")]
    pub key:     String,
    #[schemars(description = "Markdown content to store")]
    pub content: String,
    #[serde(default)]
    #[schemars(description = "Optional tags for retrieval")]
    pub tags:    Vec<String>,
    #[schemars(description = "Write mode: overwrite (default) | append (append to existing content)")]
    pub mode:    Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryReadParams {
    #[schemars(description = "Free-text query matched against key, tags, and content preview")]
    pub query: String,
    #[schemars(description = "Filter by tag (exact match on any tag in the entry)")]
    pub tag:   Option<String>,
    #[schemars(description = "Maximum entries to return (default: 10, max: 50)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryDeleteParams {
    #[schemars(description = "Memory key to delete (e.g. decisions/arch-v4)")]
    pub key: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct MemoryListParams {
    #[schemars(description = "Filter by tag (exact match on any tag in the entry)")]
    pub tag:   Option<String>,
    #[schemars(description = "Maximum entries to return (default: 50, max: 200)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct CheckpointListParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct TddEvidenceParams {
    #[schemars(description = "Filter to a specific node_id (default: all nodes)")]
    pub node_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct SessionEndParams {
    #[serde(default)]
    #[schemars(description = "Optional note to append to the session memory entry")]
    pub note: Option<String>,
}
