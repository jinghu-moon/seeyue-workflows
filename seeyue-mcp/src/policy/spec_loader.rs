// src/policy/spec_loader.rs
//
// Loads and caches YAML rule files used by the policy engine.
// All spec files are loaded once at startup and cached in memory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

use serde::Deserialize;

use crate::policy::types::{CommandClass, FileClass, Risk};

/// Type alias to reduce complexity in approval matrix return types.
type ApprovalMatrixPair = (HashMap<String, ApprovalClassEntry>, HashMap<String, ApprovalClassEntry>);

// ─── hooks.spec.yaml structures ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HooksSpec {
    pub command_classification: CommandClassification,
}

#[derive(Debug, Deserialize)]
pub struct CommandClassification {
    pub priority: Vec<String>,
    pub classes: HashMap<String, CommandClassDef>,
}

#[derive(Debug, Deserialize)]
pub struct CommandClassDef {
    pub patterns: Vec<PatternEntry>,
}

#[derive(Debug, Deserialize)]
pub struct PatternEntry {
    pub regex: String,
    pub label: String,
}

// ─── file-classes.yaml structures ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FileClassesSpec {
    pub classes: HashMap<String, FileClassDef>,
    pub match_precedence: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileClassDef {
    pub default_risk_class: String,
    pub patterns: Vec<String>,
}

// ─── approval-matrix.yaml structures ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApprovalMatrixSpec {
    pub command_classes: HashMap<String, ApprovalClassEntry>,
    pub file_classes: HashMap<String, ApprovalClassEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalClassEntry {
    pub risk_class: String,
    pub approval_required: bool,
    #[serde(default)]
    pub approval_mode: Option<String>,
    #[serde(default)]
    pub allowed_grant_scopes: Option<Vec<String>>,
    #[serde(default)]
    pub notify_only_allowed: Option<bool>,
}

// ─── Loaded & parsed policy data ─────────────────────────────────────────────

/// Compiled regex rule for command classification.
#[derive(Debug)]
pub struct CompiledCommandRule {
    pub class: CommandClass,
    pub regex: regex::Regex,
    pub label: String,
}

/// Compiled glob rule for file classification.
#[derive(Debug)]
pub struct CompiledFileRule {
    pub class: FileClass,
    pub risk: Risk,
    pub matcher: globset::GlobMatcher,
    pub pattern: String,
}

/// All policy specs loaded and compiled, ready for evaluation.
pub struct PolicySpecs {
    pub command_rules: Vec<CompiledCommandRule>,
    pub file_rules: Vec<CompiledFileRule>,
    pub command_approval: HashMap<String, ApprovalClassEntry>,
    pub file_approval: HashMap<String, ApprovalClassEntry>,
    pub workflow_dir: PathBuf,
}

impl PolicySpecs {
    /// Load all spec files from the project root.
    /// Falls back gracefully if files are missing (returns empty rules).
    pub fn load(project_root: &Path) -> Result<Self, String> {
        let workflow_dir = project_root.join("workflow");

        // ── Load command classification from hooks.spec.yaml ──
        let command_rules = Self::load_command_rules(&workflow_dir)?;

        // ── Load file classification from file-classes.yaml ──
        let file_rules = Self::load_file_rules(&workflow_dir)?;

        // ── Load approval matrix ──
        let (command_approval, file_approval) = Self::load_approval_matrix(&workflow_dir)?;

        Ok(Self {
            command_rules,
            file_rules,
            command_approval,
            file_approval,
            workflow_dir,
        })
    }

    /// Create empty specs (permissive mode — no rules loaded).
    pub fn load_empty() -> Self {
        Self {
            command_rules: vec![],
            file_rules: vec![],
            command_approval: HashMap::new(),
            file_approval: HashMap::new(),
            workflow_dir: PathBuf::new(),
        }
    }

    fn load_command_rules(workflow_dir: &Path) -> Result<Vec<CompiledCommandRule>, String> {
        let path = workflow_dir.join("hooks.spec.yaml");
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Ok(vec![]),
        };

        let spec: HooksSpec = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse hooks.spec.yaml: {}", e))?;

        let mut rules = Vec::new();

        // Iterate in priority order
        for class_name in &spec.command_classification.priority {
            let class = parse_command_class(class_name);
            if let Some(def) = spec.command_classification.classes.get(class_name) {
                for pattern in &def.patterns {
                    let re = regex::Regex::new(&pattern.regex)
                        .map_err(|e| format!("Invalid regex '{}': {}", pattern.regex, e))?;
                    rules.push(CompiledCommandRule {
                        class,
                        regex: re,
                        label: pattern.label.clone(),
                    });
                }
            }
        }

        Ok(rules)
    }

    fn load_file_rules(workflow_dir: &Path) -> Result<Vec<CompiledFileRule>, String> {
        let path = workflow_dir.join("file-classes.yaml");
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Ok(vec![]),
        };

        let spec: FileClassesSpec = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse file-classes.yaml: {}", e))?;

        let mut rules = Vec::new();

        // Iterate in match_precedence order
        for class_name in &spec.match_precedence {
            let class = parse_file_class(class_name);
            if let Some(def) = spec.classes.get(class_name) {
                let risk = parse_risk(&def.default_risk_class);
                for pattern in &def.patterns {
                    let glob = globset::Glob::new(pattern)
                        .map_err(|e| format!("Invalid glob '{}': {}", pattern, e))?
                        .compile_matcher();
                    rules.push(CompiledFileRule {
                        class,
                        risk,
                        matcher: glob,
                        pattern: pattern.clone(),
                    });
                }
            }
        }

        Ok(rules)
    }

    fn load_approval_matrix(
        workflow_dir: &Path,
    ) -> Result<ApprovalMatrixPair, String>
    {
        let path = workflow_dir.join("approval-matrix.yaml");
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Ok((HashMap::new(), HashMap::new())),
        };

        let spec: ApprovalMatrixSpec = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse approval-matrix.yaml: {}", e))?;

        Ok((spec.command_classes, spec.file_classes))
    }
}

// ─── Parse helpers ───────────────────────────────────────────────────────────

fn parse_command_class(s: &str) -> CommandClass {
    match s {
        "destructive"       => CommandClass::Destructive,
        "privileged"        => CommandClass::Privileged,
        "schema_mutation"   => CommandClass::SchemaMutation,
        "data_mutation"     => CommandClass::DataMutation,
        "git_mutating"      => CommandClass::GitMutating,
        "network_sensitive" => CommandClass::NetworkSensitive,
        "verify"            => CommandClass::Verify,
        _                   => CommandClass::Safe,
    }
}

fn parse_file_class(s: &str) -> FileClass {
    match s {
        "secret_material"      => FileClass::SecretMaterial,
        "security_boundary"    => FileClass::SecurityBoundary,
        "system_file"          => FileClass::SystemFile,
        "critical_policy_file" => FileClass::CriticalPolicyFile,
        "generated_file"       => FileClass::GeneratedFile,
        "test_file"            => FileClass::TestFile,
        "docs_file"            => FileClass::DocsFile,
        _                      => FileClass::WorkspaceFile,
    }
}

fn parse_risk(s: &str) -> Risk {
    match s {
        "low"      => Risk::Low,
        "medium"   => Risk::Medium,
        "high"     => Risk::High,
        "critical" => Risk::Critical,
        _          => Risk::Low,
    }
}
