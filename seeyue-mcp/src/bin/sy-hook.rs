// src/bin/sy-hook.rs
//
// Single Rust binary replacing all Node.js hook scripts.
//
// Usage:
//   sy-hook.exe <EVENT>          (event type as CLI argument — primary)
//   sy-hook.exe                  (infer event from stdin JSON — fallback)
//
// Protocol:
//   stdin  → JSON (HookInput: tool_input, tool_response, cwd, ...)
//   stdout → JSON (HookOutput: verdict, reason, instructions, ...)
//   exit   → 0 (allow / force_continue) | 2 (block / block_with_approval_request)
//
// Design decisions:
//   - Pure synchronous (no tokio runtime) — minimizes cold-start overhead.
//   - Fail-open on all errors — returns allow + exit(0) if anything goes wrong.
//   - Reuses PolicyEngine and workflow state from the library crate.
//   - DRY_RUN mode: SY_HOOK_DRY_RUN=1 → always returns allow (observation mode).

use std::path::Path;

use seeyue_mcp::hooks::protocol::{HookInput, emit_allow};
use seeyue_mcp::hooks::router;
use seeyue_mcp::policy::evaluator::PolicyEngine;
use seeyue_mcp::policy::spec_loader::PolicySpecs;
use seeyue_mcp::workflow::state;

fn main() {
    // Fail-open wrapper: any panic or error → allow + exit(0).
    let result = std::panic::catch_unwind(run);
    if let Err(e) = result {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        emit_allow(&format!("hook_panic — fail open: {}", msg));
    }
}

fn run() {
    // 1. Read stdin (one-shot, blocking)
    let input = HookInput::from_stdin();

    // 2. DRY_RUN mode
    if std::env::var("SY_HOOK_DRY_RUN").unwrap_or_default() == "1" {
        emit_allow("dry_run_mode");
    }

    // 3. Determine hook_event
    //
    // Priority:
    //   a) CLI argument (settings.json passes event type, e.g. `sy-hook.exe SessionStart`)
    //   b) Explicit `hook_event` field in stdin JSON
    //   c) Infer from tool_name + presence of tool_response
    //   d) Check for prompt/message fields → UserPromptSubmit
    //   e) Fallback to "Unknown" (fail-open)
    let hook_event = resolve_hook_event(&input);

    // 4. Resolve project root from CWD
    let cwd = input.resolve_cwd();
    let project_root = Path::new(&cwd);

    // 5. Load PolicySpecs (hot path — ~2-5ms release)
    let specs = PolicySpecs::load(project_root).unwrap_or_else(|_| PolicySpecs::load_empty());
    let engine = PolicyEngine::new(specs);

    // 6. Load session state
    let workflow_dir = project_root.join(".ai/workflow");
    let session = state::load_session(&workflow_dir);

    // 7. Dispatch to handler (never returns — calls exit)
    router::dispatch(&hook_event, &input, &engine, &session, &workflow_dir);
}

/// Resolve the hook event name.
///
/// Priority:
/// 1. CLI argument (argv[1]) — most reliable, set in settings.json
/// 2. Explicit `hook_event` field in stdin JSON
/// 3. Infer from tool_name + presence of tool_response (Pre vs Post)
/// 4. Check for prompt/message fields → UserPromptSubmit
/// 5. Fallback to "Unknown"
fn resolve_hook_event(input: &HookInput) -> String {
    // 1. CLI argument — authoritative source from settings.json
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && !args[1].is_empty() {
        return args[1].clone();
    }

    // 2. Explicit field in stdin JSON
    if let Some(ref event) = input.hook_event {
        if !event.is_empty() {
            return event.clone();
        }
    }

    // 3. Infer from tool_name
    if let Some(ref tool_name) = input.tool_name {
        let is_post = input.tool_response.is_some();
        let prefix = if is_post { "PostToolUse" } else { "PreToolUse" };
        return format!("{}:{}", prefix, tool_name);
    }

    // 4. Check extra fields for hints
    if input.prompt.is_some() || input.message.is_some() {
        return "UserPromptSubmit".to_string();
    }

    "Unknown".to_string()
}
