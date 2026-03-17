// src/policy/evaluator.rs
//
// Policy evaluator: decision tree for pretool_bash, pretool_write, and stop hooks.
// Combines command classification, file classification, approval matrix lookup,
// and workflow state checks to produce a Verdict.

use crate::policy::command::{self, ClassifyResult};
use crate::policy::file_class;
use crate::policy::spec_loader::{ApprovalClassEntry, PolicySpecs};
use crate::policy::types::*;
use crate::workflow::state::SessionState;

// ─── Policy Engine ───────────────────────────────────────────────────────────

/// The policy engine holds compiled specs and provides evaluation methods.
pub struct PolicyEngine {
    pub specs: PolicySpecs,
}

impl PolicyEngine {
    pub fn new(specs: PolicySpecs) -> Self {
        Self { specs }
    }

    // ── PreTool: Bash ────────────────────────────────────────────────────

    /// Evaluate a bash command before execution.
    ///
    /// Decision tree:
    /// 1. Bypass check (SY_BYPASS_PRETOOL_BASH env)
    /// 2. Git commit/push special rules
    /// 3. Classify command → command_class
    /// 4. Lookup approval matrix → risk, approval_required
    /// 5. Check loop budget
    /// 6. Verdict
    pub fn check_bash(&self, cmd: &str, session: &SessionState) -> HookResult {
        // 1. Bypass
        if std::env::var("SY_BYPASS_PRETOOL_BASH").unwrap_or_default() == "1" {
            return HookResult::allow("Bypassed via SY_BYPASS_PRETOOL_BASH");
        }

        // 2. Git special rules
        if let Some(reason) = command::check_git_special_rules(cmd) {
            return HookResult::block(&reason)
                .with_command_class(CommandClass::GitMutating)
                .with_risk(Risk::High);
        }

        // 2b. Persona command permission: reviewer personas MUST NOT run destructive commands
        if let Some(persona) = session.node.owner_persona.as_deref() {
            let cr_preview = command::classify_command(cmd, &self.specs);
            if let Some(block) = self.check_persona_command(persona, cmd, &cr_preview.class) {
                return block
                    .with_command_class(cr_preview.class)
                    .with_risk(Risk::High);
            }
        }

        // 3. Classify
        let cr: ClassifyResult = command::classify_command(cmd, &self.specs);

        // 4. Approval matrix lookup
        let entry = self.lookup_command_approval(&cr.class);

        let mut result = if entry.approval_required {
            // Check if approval has been granted (simplified: check session grants)
            if self.has_command_approval(session, &cr) {
                HookResult::allow(format!(
                    "Command class '{}' approved (label: {})",
                    cr.class, cr.label
                ))
            } else {
                HookResult::block_with_approval(format!(
                    "Command class '{}' requires approval (label: {}, risk: {})",
                    cr.class, cr.label, entry.risk
                ))
            }
        } else {
            HookResult::allow(format!(
                "Command class '{}' allowed (label: {})",
                cr.class, cr.label
            ))
        };

        result = result
            .with_command_class(cr.class)
            .with_risk(entry.risk)
            .with_label(cr.label);

        // 5. Loop budget check (only if not already blocked)
        if result.verdict == Verdict::Allow {
            if let Some(reason) = crate::workflow::state::check_loop_budget(session) {
                return HookResult::block(&reason)
                    .with_command_class(cr.class)
                    .with_risk(entry.risk);
            }
        }

        result
    }

    // ── PreTool: Write/Edit ──────────────────────────────────────────────

    /// Evaluate a file write/edit before execution.
    ///
    /// Decision tree:
    /// 1. Bypass check
    /// 2. Classify file → file_class
    /// 3. Secret material → Block
    /// 4. Persona isolation check (reviewer/planner/reader MUST NOT write files)
    /// 5. Protected files check (from policy.spec.yaml)
    /// 6. Phase/Node validity check (phase completed or no active_id → block)
    /// 7. Approval matrix lookup
    /// 8. TDD state check (for production code)
    /// 9. Scope drift check
    /// 10. Verdict
    pub fn check_write(&self, path: &str, session: &SessionState) -> HookResult {
        // 1. Bypass
        if std::env::var("SY_BYPASS_PRETOOL_WRITE").unwrap_or_default() == "1" {
            return HookResult::allow("Bypassed via SY_BYPASS_PRETOOL_WRITE");
        }

        // 2. Classify file
        let fr = file_class::classify_file(path, &self.specs);

        // 3. Secret material → always block
        if fr.class == FileClass::SecretMaterial {
            return HookResult::block(format!(
                "Writing to secret material is forbidden: {} (matched: {:?})",
                path,
                fr.matched_pattern
            ))
            .with_file_class(fr.class)
            .with_risk(Risk::Critical);
        }

        // 4. Persona isolation: non-author personas MUST NOT write files
        if let Some(persona) = session.node.owner_persona.as_deref() {
            if let Some(block) = self.check_persona_write(persona) {
                return block.with_file_class(fr.class).with_risk(Risk::High);
            }
        }

        // 5. Phase/Node validity: block production writes when phase completed or no active node
        if file_class::is_production_code(path) {
            if let Some(block) = self.check_phase_node_validity(session, path) {
                return block.with_file_class(fr.class).with_risk(Risk::High);
            }
        }
        let entry = self.lookup_file_approval(&fr.class);

        let mut result = if entry.approval_required {
            if self.has_file_approval(session, path, &fr) {
                HookResult::allow(format!(
                    "File class '{}' approved for: {}",
                    fr.class, path
                ))
            } else {
                HookResult::block_with_approval(format!(
                    "File class '{}' requires approval for: {} (risk: {})",
                    fr.class, path, entry.risk
                ))
            }
        } else {
            HookResult::allow(format!(
                "File class '{}' allowed for: {}",
                fr.class, path
            ))
        };

        result = result
            .with_file_class(fr.class)
            .with_risk(entry.risk);

        // 5. TDD exception check: if tdd_exception exists but user_approved != true, block.
        // policy.spec.yaml: must_block_until_user_approved == true
        if result.verdict == Verdict::Allow && file_class::is_production_code(path) {
            if let Some(exc) = &session.node.tdd_exception {
                let user_approved = exc
                    .get("user_approved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !user_approved {
                    let reason = exc
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("no reason provided");
                    return HookResult::block(format!(
                        "Production write blocked: tdd_exception exists but user_approved is false. \
                         Reason: {}",
                        reason
                    ))
                    .with_file_class(fr.class)
                    .with_risk(entry.risk)
                    .with_instructions(vec![
                        "TDD exception requires explicit user approval before proceeding.".to_string(),
                        format!("Exception reason: {}", reason),
                        "Set tdd_exception.user_approved: true via sy_advance_node to unblock.".to_string(),
                    ]);
                }
            }
        }

        // 6. TDD state check (only for production code that isn't already blocked)
        if result.verdict == Verdict::Allow
            && file_class::is_production_code(path)
            && !crate::workflow::state::check_tdd_ready(session)
        {
            let tdd_state = session
                .node
                .tdd_state
                .as_deref()
                .unwrap_or("unknown");

            return HookResult::block(format!(
                "Production write blocked: TDD state '{}' does not allow writes. \
                 Write tests first (need red_verified or later).",
                tdd_state
            ))
            .with_file_class(fr.class)
            .with_risk(entry.risk)
            .with_instructions(vec![
                "Write or update test files first".to_string(),
                "Run red_cmd to verify test fails".to_string(),
                "Then proceed with production code".to_string(),
            ]);
        }

        // 7. Scope drift check (only for production code that isn't already blocked)
        if result.verdict == Verdict::Allow && file_class::is_production_code(path) {
            if let Some(instructions) = self.check_scope_drift(session, path) {
                result = result.with_instructions(instructions);
            }
        }

        result
    }

    // ── Stop ─────────────────────────────────────────────────────────────

    /// Evaluate whether the session can stop.
    ///
    /// Decision tree:
    /// 1. Loop budget exhausted → ForceContinue
    /// 2. Pending approvals → ForceContinue
    /// 3. Restore pending → ForceContinue
    /// 4. Allow
    pub fn check_stop(&self, session: &SessionState) -> HookResult {
        // 1. Loop budget
        if let Some(reason) = crate::workflow::state::check_loop_budget(session) {
            return HookResult::force_continue(reason)
                .with_instructions(vec![
                    "Budget exhausted — request human takeover".to_string(),
                ]);
        }

        // 2. Pending approvals
        if crate::workflow::state::has_pending_approvals(session) {
            return HookResult::force_continue(
                "Cannot stop: pending approvals must be resolved first",
            )
            .with_instructions(vec![
                "Resolve pending approval requests before stopping".to_string(),
            ]);
        }

        // 3. Restore pending
        if crate::workflow::state::is_restore_pending(session) {
            let reason = session
                .recovery
                .restore_reason
                .as_deref()
                .unwrap_or("unknown reason");
            return HookResult::force_continue(format!(
                "Cannot stop: restore pending ({})",
                reason
            ))
            .with_instructions(vec![
                "Resolve recovery state before stopping".to_string(),
            ]);
        }

        // 4. Allow
        HookResult::allow("Stop allowed")
    }

    // ── Approval lookup helpers ──────────────────────────────────────────

    fn lookup_command_approval(&self, class: &CommandClass) -> ApprovalEntry {
        let key = class.to_string();
        match self.specs.command_approval.get(&key) {
            Some(entry) => to_approval_entry(entry),
            None => ApprovalEntry {
                risk: Risk::Low,
                approval_required: false,
                approval_mode: None,
                allowed_grant_scopes: vec![],
                notify_only_allowed: true,
            },
        }
    }

    fn lookup_file_approval(&self, class: &FileClass) -> ApprovalEntry {
        let key = class.to_string();
        match self.specs.file_approval.get(&key) {
            Some(entry) => to_approval_entry(entry),
            None => ApprovalEntry {
                risk: Risk::Low,
                approval_required: false,
                approval_mode: None,
                allowed_grant_scopes: vec![],
                notify_only_allowed: true,
            },
        }
    }

    /// Check if the session has a grant that covers this command.
    /// Simplified: checks session.approvals.grants for matching entries.
    fn has_command_approval(&self, session: &SessionState, cr: &ClassifyResult) -> bool {
        let grants = match &session.approvals.grants {
            Some(g) => g,
            None => return false,
        };

        let class_str = cr.class.to_string();

        for grant in grants {
            // Match by command_class field
            if let Some(action) = grant.get("action").and_then(|v| v.as_str()) {
                if action == class_str || action == cr.label {
                    return true;
                }
            }
            // Match by command_class field directly
            if let Some(cc) = grant.get("command_class").and_then(|v| v.as_str()) {
                if cc == class_str {
                    return true;
                }
            }
        }

        false
    }

    /// Check if the session has a grant that covers this file write.
    fn has_file_approval(
        &self,
        session: &SessionState,
        path: &str,
        fr: &file_class::FileClassifyResult,
    ) -> bool {
        let grants = match &session.approvals.grants {
            Some(g) => g,
            None => return false,
        };

        let class_str = fr.class.to_string();

        for grant in grants {
            // Match by file_class
            if let Some(fc) = grant.get("file_class").and_then(|v| v.as_str()) {
                if fc == class_str {
                    return true;
                }
            }
            // Match by target_ref (exact path)
            if let Some(target) = grant.get("target_ref").and_then(|v| v.as_str()) {
                if target == path {
                    return true;
                }
            }
        }

        false
    }

    /// Check for scope drift: is the file outside the node's target scope?
    /// Returns warning instructions if drift detected, None otherwise.
    fn check_scope_drift(&self, session: &SessionState, path: &str) -> Option<Vec<String>> {
        let targets = session.node.target.as_ref()?;
        if targets.is_empty() {
            return None;
        }

        // Check if path is within any target scope
        let normalized = path.replace('\\', "/");
        let in_scope = targets.iter().any(|t| {
            let t_norm = t.replace('\\', "/");
            normalized.starts_with(&t_norm) || t_norm.contains("*")
        });

        if !in_scope {
            Some(vec![format!(
                "⚠️ Scope drift: '{}' is outside node target scope {:?}. \
                 Verify this change is necessary for the current task.",
                path, targets
            )])
        } else {
            None
        }
    }

    // ── P1: Persona isolation ────────────────────────────────────────────

    /// Block file writes for personas that are not allowed to write files.
    fn check_persona_write(&self, persona: &str) -> Option<HookResult> {
        let def = self.specs.persona_bindings.personas.get(persona)?;
        if def.may_write_files == Some(false) {
            return Some(
                HookResult::block(format!(
                    "Persona '{}' may not write files. Switch to author persona.",
                    persona
                ))
                .with_instructions(vec![
                    format!("Current persona: {}", persona),
                    "Required: author persona for file writes".to_string(),
                ]),
            );
        }
        None
    }

    /// Block destructive commands for reviewer/planner/reader personas.
    fn check_persona_command(
        &self,
        persona: &str,
        cmd: &str,
        class: &CommandClass,
    ) -> Option<HookResult> {
        // Hardcoded reviewer guard — enforced regardless of persona_bindings spec.
        // Checks the classified class AND a builtin regex so this guard works even
        // when specs are empty (e.g. load_empty() in tests).
        let reviewer_personas = ["spec_reviewer", "quality_reviewer"];
        if reviewer_personas.contains(&persona) {
            let destructive_classes = [
                CommandClass::Destructive,
                CommandClass::GitMutating,
                CommandClass::Privileged,
            ];
            let is_dangerous = destructive_classes.contains(class)
                || command::is_builtin_dangerous(cmd);
            if is_dangerous {
                return Some(
                    HookResult::block(format!(
                        "Reviewer persona '{}' may not run {:?} commands.",
                        persona, class
                    ))
                    .with_instructions(vec![
                        "Reviewers are read-only for destructive/git/privileged commands".to_string(),
                    ]),
                );
            }
        }

        // Spec-driven check: may_run_commands: false
        let def = self.specs.persona_bindings.personas.get(persona)?;
        if def.may_run_commands == Some(false) {
            return Some(HookResult::block(format!(
                "Persona '{}' may not run commands.",
                persona
            )));
        }
        None
    }

    /// Always-dangerous check independent of specs.
    #[allow(dead_code)]
    fn matches_builtin_dangerous(class: &CommandClass) -> bool {
        matches!(
            class,
            CommandClass::Destructive | CommandClass::GitMutating | CommandClass::Privileged
        )
    }

    // ── P1: Phase/Node validity ──────────────────────────────────────────

    /// Block production writes when phase is completed or no active node in execute phase.
    fn check_phase_node_validity(
        &self,
        session: &SessionState,
        path: &str,
    ) -> Option<HookResult> {
        let phase_status = session.phase.status.as_deref().unwrap_or("");
        let phase_id = session
            .phase
            .id
            .as_deref()
            .or(session.phase.name.as_deref())
            .unwrap_or("");

        // Block if phase is marked completed
        if phase_status == "completed" && !phase_id.is_empty() {
            return Some(
                HookResult::block(format!(
                    "Phase '{}' is completed — production writes are blocked.",
                    phase_id
                ))
                .with_instructions(vec![
                    format!("File: {}", path),
                    format!("Phase: {} ({})", phase_id, phase_status),
                    "Start a new execution phase before writing code.".to_string(),
                ]),
            );
        }

        // In execute phase: require active node
        let phase_name = session
            .phase
            .name
            .as_deref()
            .or(session.phase.id.as_deref())
            .unwrap_or("");
        if phase_name == "execute" {
            let node_id = session
                .node
                .id
                .as_deref()
                .or(session.node.name.as_deref())
                .unwrap_or("");
            if node_id.is_empty() {
                return Some(
                    HookResult::block(
                        "No active node in execute phase — production writes are blocked.",
                    )
                    .with_instructions(vec![
                        format!("File: {}", path),
                        "Use sy_advance_node to activate a plan node first.".to_string(),
                    ]),
                );
            }
            let node_status = session.node.status.as_deref().unwrap_or("");
            if !node_status.is_empty()
                && !matches!(node_status, "in_progress" | "blocked" | "review")
            {
                return Some(
                    HookResult::block(format!(
                        "Node status '{}' does not allow production writes. \
                         Allowed: in_progress, blocked, review.",
                        node_status
                    ))
                    .with_instructions(vec![
                        format!("File: {}", path),
                        format!("Node: {} ({})", node_id, node_status),
                    ]),
                );
            }
        }

        None
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn to_approval_entry(e: &ApprovalClassEntry) -> ApprovalEntry {
    ApprovalEntry {
        risk: match e.risk_class.as_str() {
            "low" => Risk::Low,
            "medium" => Risk::Medium,
            "high" => Risk::High,
            "critical" => Risk::Critical,
            _ => Risk::Low,
        },
        approval_required: e.approval_required,
        approval_mode: e.approval_mode.clone(),
        allowed_grant_scopes: e
            .allowed_grant_scopes
            .clone()
            .unwrap_or_default(),
        notify_only_allowed: e.notify_only_allowed.unwrap_or(false),
    }
}
