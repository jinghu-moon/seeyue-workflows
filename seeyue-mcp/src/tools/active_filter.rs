// src/tools/active_filter.rs
// Active tool filter (M-N5).
// Determines whether a tool call is permitted based on active_tools set
// and the tool's active_by_default metadata flag.

use std::collections::HashSet;

/// Result of an active-filter check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterResult {
    /// Tool is permitted to execute.
    Allowed,
    /// Tool exists but is not active in this session.
    Disabled,
}

/// Stateless filter backed by an owned active_tools set.
/// Created once at dispatch time from AppState; check() is synchronous.
pub struct ActiveFilter {
    active_tools: HashSet<String>,
}

impl ActiveFilter {
    pub fn new(active_tools: HashSet<String>) -> Self {
        Self { active_tools }
    }

    /// Check whether `name` is permitted.
    /// Allowed if: explicitly in active_tools OR active_by_default=true in registry.
    pub fn check(&self, name: &str) -> FilterResult {
        if crate::tools::metadata::ToolMetadata::is_active(name, &self.active_tools) {
            FilterResult::Allowed
        } else {
            FilterResult::Disabled
        }
    }
}

/// Load active_tools list from a runtime capabilities YAML file.
/// Expected format: `active_tools: ["tool_a", "tool_b"]`
/// Returns empty set if file is missing, unreadable, or has no active_tools key.
pub fn load_active_tools_from_yaml(path: &str) -> HashSet<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashSet::new(),
    };

    // Simple YAML parse: look for `active_tools:` list items (`  - tool_name`)
    let mut set = HashSet::new();
    let mut in_active_tools = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("active_tools:") {
            in_active_tools = true;
            // Inline list: active_tools: [a, b, c]
            if let Some(bracket_start) = trimmed.find('[') {
                let inner = &trimmed[bracket_start + 1..];
                let inner = inner.trim_end_matches(']');
                for item in inner.split(',') {
                    let name = item.trim().trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        set.insert(name.to_string());
                    }
                }
                in_active_tools = false;
            }
            continue;
        }
        if in_active_tools {
            if trimmed.starts_with('-') {
                let name = trimmed[1..].trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    set.insert(name.to_string());
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                // New key — end of list
                in_active_tools = false;
            }
        }
    }
    set
}
