// src/hooks/prompt_refresh.rs
//
// UserPromptSubmit handler: lightweight constraint anchor injection.
//
// Checks if the session is in an active workflow phase and if the user prompt
// contains trigger keywords. If both conditions are met, injects a constraint
// anchor into the context to remind the model of workflow obligations.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::hooks::protocol::{HookInput, emit_allow, emit_allow_with_extra};
use crate::workflow::state::SessionState;

/// Default phases where prompt refresh is active.
const ACTIVE_PHASES: &[&str] = &["plan", "execute", "review"];

/// Default trigger keywords that activate constraint anchor injection.
const TRIGGER_KEYWORDS: &[&str] = &[
    "implement",
    "write",
    "create",
    "add",
    "fix",
    "update",
    "refactor",
    "delete",
    "remove",
    "modify",
    "change",
];

/// Handle the UserPromptSubmit hook event.
pub fn handle(input: &HookInput, _workflow_dir: &Path, session: &SessionState) -> ! {
    // 1. Bypass check
    if std::env::var("SY_BYPASS_PROMPT_REFRESH").unwrap_or_default() == "1" {
        emit_allow("prompt_refresh_bypass");
    }

    // 2. Check if session has an active phase
    let phase = match session.phase.name.as_deref().or(session.phase.id.as_deref()) {
        Some(p) if !p.is_empty() => p,
        _ => emit_allow("prompt_refresh_inactive"),
    };

    // 3. Check if phase is in active set
    let phase_lower = phase.to_lowercase();
    if !ACTIVE_PHASES.iter().any(|&p| p == phase_lower) {
        emit_allow("prompt_refresh_inactive");
    }

    // 4. Check prompt for trigger keywords
    let prompt = input.resolve_prompt().to_lowercase();
    let matches = TRIGGER_KEYWORDS.iter().any(|&kw| prompt.contains(kw));
    if !matches {
        emit_allow("prompt_refresh_no_match");
    }

    // 5. Build constraint anchor
    let anchor = format!(
        "[sy-constraints] Active workflow phase: {}. \
         Load relevant child constraint skill BEFORE writing any code. \
         Hooks enforce dangerous-command and secrets guards. Do not pre-empt them.",
        phase
    );

    let mut extra = HashMap::new();
    extra.insert("context".to_string(), Value::String(anchor));

    emit_allow_with_extra("prompt_refresh", extra)
}
