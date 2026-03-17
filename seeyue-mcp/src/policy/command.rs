// src/policy/command.rs
//
// Command classifier: matches shell commands against regex patterns
// loaded from `workflow/hooks.spec.yaml` → `command_classification`.
//
// Priority order: destructive > privileged > schema_mutation > data_mutation
//                 > git_mutating > network_sensitive > verify > safe
//
// Special rules:
//   - `git commit --dry-run` / `git push --dry-run` → Safe
//   - `git commit` without SY_ALLOW_GIT_COMMIT=1 → Block
//   - `git push` without SY_ALLOW_GIT_PUSH=1 → Block

use crate::policy::spec_loader::PolicySpecs;
use crate::policy::types::CommandClass;

/// Result of classifying a command.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClassifyResult {
    pub class: CommandClass,
    pub label: String,
    pub matched_pattern: Option<String>,
}

/// Classify a shell command string using compiled regex rules.
///
/// Returns the highest-priority matching class (rules are pre-sorted by priority).
/// If no rule matches, returns `Safe`.
pub fn classify_command(command: &str, specs: &PolicySpecs) -> ClassifyResult {
    // Special case: dry-run git commands are safe
    if is_git_dry_run(command) {
        return ClassifyResult {
            class: CommandClass::Safe,
            label: "git_dry_run".to_string(),
            matched_pattern: None,
        };
    }

    // Walk rules in priority order (destructive first)
    for rule in &specs.command_rules {
        if rule.regex.is_match(command) {
            return ClassifyResult {
                class: rule.class,
                label: rule.label.clone(),
                matched_pattern: Some(rule.regex.as_str().to_string()),
            };
        }
    }

    // Default: safe
    ClassifyResult {
        class: CommandClass::Safe,
        label: "unclassified".to_string(),
        matched_pattern: None,
    }
}

/// Check if this is a git commit or push that requires special env var authorization.
/// Returns `Some(reason)` if blocked, `None` if allowed.
pub fn check_git_special_rules(command: &str) -> Option<String> {
    // Skip dry-run
    if is_git_dry_run(command) {
        return None;
    }

    let git_commit_re = regex::Regex::new(r"\bgit\s+commit\b").unwrap();
    let git_push_re = regex::Regex::new(r"\bgit\s+push\b").unwrap();

    if git_commit_re.is_match(command)
        && std::env::var("SY_ALLOW_GIT_COMMIT").unwrap_or_default() != "1"
    {
        return Some(
            "git commit requires SY_ALLOW_GIT_COMMIT=1 or explicit approval".to_string(),
        );
    }

    if git_push_re.is_match(command)
        && std::env::var("SY_ALLOW_GIT_PUSH").unwrap_or_default() != "1"
    {
        return Some(
            "git push requires SY_ALLOW_GIT_PUSH=1 or explicit approval".to_string(),
        );
    }

    None
}

/// Check if command is a dry-run git operation (safe to allow).
fn is_git_dry_run(command: &str) -> bool {
    let dry_run_re = regex::Regex::new(r"\bgit\s+(commit|push)\b.*--dry-run").unwrap();
    dry_run_re.is_match(command)
}

/// Builtin dangerous command detection — independent of loaded specs.
/// Used as a fallback when specs are empty (e.g. in unit tests).
/// Covers the most common destructive/privileged/git-mutating patterns.
pub fn is_builtin_dangerous(cmd: &str) -> bool {
    static DANGEROUS: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| {
            regex::Regex::new(
                r"(?x)
                \brm\s+(-[^\s]*f|-[^\s]*r|--force|--recursive)   # rm -rf / rm -f
                | \bdel\s+/[sqfQ]                                  # Windows del /s /q
                | \bformat\b                                        # format drive
                | \bmkfs\b                                          # make filesystem
                | \bdd\b.*\bof=                                     # dd to device
                | \bsudo\b                                          # privileged
                | \bchmod\s+[0-7]*7[0-7]*                          # chmod 777
                | \bchown\b.*\s/                                    # chown root paths
                | \bgit\s+push\b                                    # git push
                | \bgit\s+commit\b                                  # git commit
                | \bgit\s+reset\s+--hard\b                         # git reset --hard
                | \bgit\s+clean\s+-[^\s]*f                         # git clean -f
                ",
            )
            .unwrap()
        });
    DANGEROUS.is_match(cmd)
}

#[cfg(test)]
mod tests {
    // Unit tests require PolicySpecs loaded from actual YAML files.
    // See test_e2e_p1.py for integration tests.
}
