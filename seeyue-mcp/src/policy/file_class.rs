// src/policy/file_class.rs
//
// File classifier: matches file paths against glob patterns
// loaded from `workflow/file-classes.yaml`.
//
// Match precedence (first match wins):
//   secret_material > security_boundary > system_file > critical_policy_file
//   > generated_file > test_file > docs_file > workspace_file

use crate::policy::spec_loader::PolicySpecs;
use crate::policy::types::{FileClass, Risk};

/// Result of classifying a file path.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FileClassifyResult {
    pub class: FileClass,
    pub risk: Risk,
    pub matched_pattern: Option<String>,
}

/// Classify a file path using compiled glob rules.
///
/// The path should be relative to the project root, using forward slashes.
/// Returns the highest-precedence matching class.
/// If no rule matches, returns `WorkspaceFile` with `Low` risk.
pub fn classify_file(path: &str, specs: &PolicySpecs) -> FileClassifyResult {
    // Normalize path: backslash → forward slash, strip leading ./
    let normalized = normalize_path(path);

    // Walk rules in precedence order (secret_material first)
    for rule in &specs.file_rules {
        if rule.matcher.is_match(&normalized) {
            return FileClassifyResult {
                class: rule.class,
                risk: rule.risk,
                matched_pattern: Some(rule.pattern.clone()),
            };
        }
    }

    // Default: workspace file
    FileClassifyResult {
        class: FileClass::WorkspaceFile,
        risk: Risk::Low,
        matched_pattern: None,
    }
}

/// Check if a path is a production code file (not test/spec/docs).
pub fn is_production_code(path: &str) -> bool {
    let normalized = normalize_path(path);

    // Exclude test files
    if normalized.contains(".test.") || normalized.contains(".spec.") {
        return false;
    }
    if normalized.starts_with("tests/") || normalized.starts_with("test/") {
        return false;
    }

    // Exclude docs
    if normalized.ends_with(".md") {
        return false;
    }
    if normalized.starts_with("docs/") {
        return false;
    }

    // Exclude generated
    if normalized.starts_with("dist/") || normalized.starts_with("coverage/") {
        return false;
    }
    if normalized.contains(".generated.") {
        return false;
    }

    // Exclude config/workflow
    if normalized.starts_with("workflow/") || normalized.starts_with(".ai/") {
        return false;
    }

    true
}

/// Normalize file path for glob matching.
fn normalize_path(path: &str) -> String {
    let mut p = path.replace('\\', "/");

    // Strip leading ./
    if p.starts_with("./") {
        p = p[2..].to_string();
    }

    // Strip leading /
    if p.starts_with('/') {
        p = p[1..].to_string();
    }

    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_production_code() {
        assert!(is_production_code("src/main.rs"));
        assert!(is_production_code("lib/utils.ts"));
        assert!(!is_production_code("tests/test_main.rs"));
        assert!(!is_production_code("src/main.test.ts"));
        assert!(!is_production_code("docs/README.md"));
        assert!(!is_production_code("workflow/policy.spec.yaml"));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("src\\main.rs"), "src/main.rs");
        assert_eq!(normalize_path("./src/main.rs"), "src/main.rs");
        assert_eq!(normalize_path("/src/main.rs"), "src/main.rs");
    }
}
