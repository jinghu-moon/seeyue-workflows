// src/policy/types.rs
//
// Shared types for the policy engine: command classification, file classification,
// risk levels, verdicts, and hook results.

use serde::{Deserialize, Serialize};
use std::fmt;

// ─── Command Class ───────────────────────────────────────────────────────────

/// Command classes ranked by severity (highest first in match priority).
/// Derived from `workflow/hooks.spec.yaml` → `command_classification.priority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandClass {
    Destructive,
    Privileged,
    SchemaMutation,
    DataMutation,
    GitMutating,
    NetworkSensitive,
    Verify,
    Safe,
}

impl CommandClass {
    /// Priority order for classification (first match wins).
    pub const PRIORITY: &'static [CommandClass] = &[
        CommandClass::Destructive,
        CommandClass::Privileged,
        CommandClass::SchemaMutation,
        CommandClass::DataMutation,
        CommandClass::GitMutating,
        CommandClass::NetworkSensitive,
        CommandClass::Verify,
        CommandClass::Safe,
    ];
}

impl fmt::Display for CommandClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandClass::Destructive     => write!(f, "destructive"),
            CommandClass::Privileged      => write!(f, "privileged"),
            CommandClass::SchemaMutation  => write!(f, "schema_mutation"),
            CommandClass::DataMutation    => write!(f, "data_mutation"),
            CommandClass::GitMutating     => write!(f, "git_mutating"),
            CommandClass::NetworkSensitive => write!(f, "network_sensitive"),
            CommandClass::Verify          => write!(f, "verify"),
            CommandClass::Safe            => write!(f, "safe"),
        }
    }
}

// ─── File Class ──────────────────────────────────────────────────────────────

/// File classes ranked by match precedence (highest priority first).
/// Derived from `workflow/file-classes.yaml` → `match_precedence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileClass {
    SecretMaterial,
    SecurityBoundary,
    SystemFile,
    CriticalPolicyFile,
    GeneratedFile,
    TestFile,
    DocsFile,
    WorkspaceFile,
}

impl FileClass {
    /// Match precedence order (first match wins).
    pub const PRECEDENCE: &'static [FileClass] = &[
        FileClass::SecretMaterial,
        FileClass::SecurityBoundary,
        FileClass::SystemFile,
        FileClass::CriticalPolicyFile,
        FileClass::GeneratedFile,
        FileClass::TestFile,
        FileClass::DocsFile,
        FileClass::WorkspaceFile,
    ];
}

impl fmt::Display for FileClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileClass::SecretMaterial      => write!(f, "secret_material"),
            FileClass::SecurityBoundary    => write!(f, "security_boundary"),
            FileClass::SystemFile          => write!(f, "system_file"),
            FileClass::CriticalPolicyFile  => write!(f, "critical_policy_file"),
            FileClass::GeneratedFile       => write!(f, "generated_file"),
            FileClass::TestFile            => write!(f, "test_file"),
            FileClass::DocsFile            => write!(f, "docs_file"),
            FileClass::WorkspaceFile       => write!(f, "workspace_file"),
        }
    }
}

// ─── Risk ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for Risk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Risk::Low      => write!(f, "low"),
            Risk::Medium   => write!(f, "medium"),
            Risk::High     => write!(f, "high"),
            Risk::Critical => write!(f, "critical"),
        }
    }
}

// ─── Verdict ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Allow,
    Block,
    BlockWithApprovalRequest,
    ForceContinue,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Allow                    => write!(f, "allow"),
            Verdict::Block                    => write!(f, "block"),
            Verdict::BlockWithApprovalRequest => write!(f, "block_with_approval_request"),
            Verdict::ForceContinue            => write!(f, "force_continue"),
        }
    }
}

// ─── Hook Result ─────────────────────────────────────────────────────────────

/// Structured result returned by all hook tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub verdict: Verdict,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instructions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl HookResult {
    pub fn allow(reason: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Allow,
            reason: reason.into(),
            instructions: vec![],
            command_class: None,
            file_class: None,
            risk: None,
            label: None,
        }
    }

    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::Block,
            reason: reason.into(),
            instructions: vec![],
            command_class: None,
            file_class: None,
            risk: None,
            label: None,
        }
    }

    pub fn block_with_approval(reason: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::BlockWithApprovalRequest,
            reason: reason.into(),
            instructions: vec![],
            command_class: None,
            file_class: None,
            risk: None,
            label: None,
        }
    }

    pub fn force_continue(reason: impl Into<String>) -> Self {
        Self {
            verdict: Verdict::ForceContinue,
            reason: reason.into(),
            instructions: vec![],
            command_class: None,
            file_class: None,
            risk: None,
            label: None,
        }
    }

    pub fn with_command_class(mut self, cc: CommandClass) -> Self {
        self.command_class = Some(cc.to_string());
        self
    }

    pub fn with_file_class(mut self, fc: FileClass) -> Self {
        self.file_class = Some(fc.to_string());
        self
    }

    pub fn with_risk(mut self, r: Risk) -> Self {
        self.risk = Some(r.to_string());
        self
    }

    pub fn with_label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    pub fn with_instructions(mut self, instructions: Vec<String>) -> Self {
        self.instructions = instructions;
        self
    }
}

// ─── Approval Matrix Entry ───────────────────────────────────────────────────

/// Lookup result from the approval matrix.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ApprovalEntry {
    pub risk: Risk,
    pub approval_required: bool,
    pub approval_mode: Option<String>,
    pub allowed_grant_scopes: Vec<String>,
    pub notify_only_allowed: bool,
}
