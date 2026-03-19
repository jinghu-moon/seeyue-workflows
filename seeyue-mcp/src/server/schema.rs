// src/server/schema.rs
// Generate tools/list entries from the ToolMetadata registry.
// Does not replace rmcp macro-generated tools; provides a validation/utility layer.

use crate::tools::metadata::registry;

/// Lightweight tool descriptor for tools/list generation and testing.
#[derive(Debug, Clone)]
pub struct ToolListEntry {
    pub name:             String,
    pub description:      String,
    pub read_only_hint:   bool,
    pub destructive_hint: bool,
}

/// Generate a tools/list from the ToolMetadata registry.
pub fn generate_tools_list() -> Vec<ToolListEntry> {
    let mut entries: Vec<ToolListEntry> = registry()
        .values()
        .map(|m| ToolListEntry {
            name:             m.name.to_string(),
            description:      m.description.to_string(),
            read_only_hint:   m.read_only,
            destructive_hint: m.destructive,
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}
