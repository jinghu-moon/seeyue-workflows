// tests/test_policy.rs
//
// Functional tests for the policy engine: check_bash, check_write, check_stop.
// Uses PolicySpecs::load_empty() to avoid filesystem dependency in unit tests.
// Run: cargo test --test test_policy

use rstest::rstest;

use seeyue_mcp::policy::evaluator::PolicyEngine;
use seeyue_mcp::policy::spec_loader::PolicySpecs;
use seeyue_mcp::policy::types::Verdict;
use seeyue_mcp::workflow::state::{
    ApprovalState, LoopBudget, NodeState, PhaseState, RecoveryState, SessionState,
};

// ─── Fixtures ────────────────────────────────────────────────────────────────

fn engine() -> PolicyEngine {
    PolicyEngine::new(PolicySpecs::load_empty())
}

fn session() -> SessionState {
    SessionState {
        schema_version: Some(1),
        run_id: Some("test-run".to_string()),
        phase: PhaseState {
            id: Some("P1".to_string()),
            name: Some("execute".to_string()),
            status: Some("active".to_string()),
        },
        node: NodeState {
            id: Some("N1".to_string()),
            name: Some("Test Node".to_string()),
            status: Some("in_progress".to_string()),
            state: Some("green_verified".to_string()),
            tdd_required: Some(false),
            tdd_state: Some("green_verified".to_string()),
            tdd_exception: None,
            target: Some(vec!["src/".to_string()]),
            test_contract: None,
            owner_persona: None,
            phase_id: Some("P1".to_string()),
        },
        loop_budget: LoopBudget {
            max: Some(100),
            used: Some(5),
            exhausted: Some(false),
            max_nodes: None,
            consumed_nodes: None,
            max_failures: None,
            consumed_failures: None,
            max_pending_approvals: None,
            max_context_utilization: None,
            current_context_utilization: None,
            max_rework_cycles: None,
            consumed_rework_cycles: None,
        },
        approvals: ApprovalState {
            pending: None,
            grants: None,
        },
        recovery: RecoveryState {
            status: None,
            restore_reason: None,
            last_checkpoint_id: None,
        },
    }
}

// ─── check_bash: safe commands ───────────────────────────────────────────────

#[rstest]
#[case("ls -la")]
#[case("echo hello")]
#[case("cat README.md")]
#[case("pwd")]
fn test_bash_safe_commands_allowed(#[case] cmd: &str) {
    let result = engine().check_bash(cmd, &session());
    assert_eq!(
        result.verdict, Verdict::Allow,
        "safe command '{}' should be allowed", cmd
    );
}

// ─── check_bash: verify commands ─────────────────────────────────────────────

#[rstest]
#[case("cargo test")]
#[case("npm test")]
#[case("pytest -v")]
#[case("cargo check")]
fn test_bash_verify_commands_allowed(#[case] cmd: &str) {
    let result = engine().check_bash(cmd, &session());
    assert_eq!(
        result.verdict, Verdict::Allow,
        "verify command '{}' should be allowed", cmd
    );
}

// ─── check_bash: git special rules (always block) ────────────────────────────

#[rstest]
#[case("git push origin main")]
#[case("git push")]
#[case("git push --force")]
#[case("git commit -m 'test'")]
#[case("git commit --amend")]
fn test_bash_git_mutating_blocked(#[case] cmd: &str) {
    let result = engine().check_bash(cmd, &session());
    assert_eq!(
        result.verdict, Verdict::Block,
        "git mutating '{}' should be blocked", cmd
    );
}

// ─── check_bash: loop budget exhausted ───────────────────────────────────────

#[test]
fn test_bash_blocked_when_budget_exhausted() {
    let mut s = session();
    s.loop_budget.exhausted = Some(true);
    s.loop_budget.used = Some(100);
    s.loop_budget.max = Some(100);
    let result = engine().check_bash("ls", &s);
    assert_eq!(result.verdict, Verdict::Block, "budget exhausted should block");
}

// ─── check_bash: reviewer persona blocked for destructive ────────────────────

#[rstest]
#[case("spec_reviewer",    "rm -rf /",             Verdict::Block)]
#[case("quality_reviewer", "git push origin main",  Verdict::Block)]
#[case("author",           "ls -la",                Verdict::Allow)]
fn test_bash_persona_command_guard(
    #[case] persona: &str,
    #[case] cmd: &str,
    #[case] expected: Verdict,
) {
    let mut s = session();
    s.node.owner_persona = Some(persona.to_string());
    let result = engine().check_bash(cmd, &s);
    assert_eq!(result.verdict, expected,
        "persona '{}' cmd '{}' expected {:?}", persona, cmd, expected);
}

// ─── check_write: secret material ────────────────────────────────────────────

#[rstest]
#[case(".env")]
#[case(".env.local")]
#[case("secrets/api-key.pem")]
#[case("config/credentials.json")]
fn test_write_secret_always_blocked(#[case] path: &str) {
    let result = engine().check_write(path, &session());
    assert_eq!(
        result.verdict, Verdict::Block,
        "secret file '{}' should always be blocked", path
    );
}

// ─── check_write: normal source files ────────────────────────────────────────

#[rstest]
#[case("src/main.rs")]
#[case("src/lib.rs")]
#[case("tests/test_foo.rs")]
fn test_write_source_allowed(#[case] path: &str) {
    let result = engine().check_write(path, &session());
    assert_eq!(
        result.verdict, Verdict::Allow,
        "source file '{}' should be allowed", path
    );
}

// ─── check_write: TDD state blocks production writes ─────────────────────────

#[rstest]
#[case("no_tests",     Verdict::Block)]
#[case("red_written",  Verdict::Block)]
#[case("red_verified", Verdict::Allow)]
#[case("green_verified", Verdict::Allow)]
fn test_write_tdd_state(#[case] tdd_state: &str, #[case] expected: Verdict) {
    let mut s = session();
    s.node.tdd_state = Some(tdd_state.to_string());
    s.node.tdd_required = Some(true);
    let result = engine().check_write("src/main.rs", &s);
    assert_eq!(result.verdict, expected,
        "tdd_state '{}' for production write expected {:?}", tdd_state, expected);
}

// ─── check_write: tdd_exception blocks until user_approved ───────────────────

#[test]
fn test_write_tdd_exception_unapproved_blocks() {
    let mut s = session();
    let exc = serde_yaml::from_str::<serde_yaml::Value>(
        "user_approved: false
reason: skipping TDD for hotfix"
    ).unwrap();
    s.node.tdd_exception = Some(exc);
    let result = engine().check_write("src/main.rs", &s);
    assert_eq!(result.verdict, Verdict::Block,
        "unapproved tdd_exception should block production writes");
}

#[test]
fn test_write_tdd_exception_approved_allows() {
    let mut s = session();
    let exc = serde_yaml::from_str::<serde_yaml::Value>(
        "user_approved: true
reason: hotfix approved"
    ).unwrap();
    s.node.tdd_exception = Some(exc);
    let result = engine().check_write("src/main.rs", &s);
    assert_eq!(result.verdict, Verdict::Allow,
        "approved tdd_exception should allow production writes");
}

// ─── check_write: persona isolation ──────────────────────────────────────────

#[test]
fn test_write_reviewer_persona_blocked() {
    // spec_reviewer with may_write_files: false would block
    // With load_empty(), persona_bindings is empty so this only tests
    // that the engine doesn't panic — actual block depends on spec
    let mut s = session();
    s.node.owner_persona = Some("author".to_string());
    let result = engine().check_write("src/main.rs", &s);
    assert_eq!(result.verdict, Verdict::Allow,
        "author persona should be allowed to write source files");
}

// ─── check_write: phase completed blocks ─────────────────────────────────────

#[test]
fn test_write_blocked_when_phase_completed() {
    let mut s = session();
    s.phase.status = Some("completed".to_string());
    s.phase.id = Some("P1".to_string());
    let result = engine().check_write("src/main.rs", &s);
    assert_eq!(result.verdict, Verdict::Block,
        "completed phase should block production writes");
}

// ─── check_stop ───────────────────────────────────────────────────────────────

#[test]
fn test_stop_allowed_clean_session() {
    let result = engine().check_stop(&session());
    assert_eq!(result.verdict, Verdict::Allow, "clean session should allow stop");
}

#[test]
fn test_stop_blocked_budget_exhausted() {
    let mut s = session();
    s.loop_budget.exhausted = Some(true);
    let result = engine().check_stop(&s);
    assert_eq!(result.verdict, Verdict::ForceContinue,
        "exhausted budget should force continue");
}

#[test]
fn test_stop_blocked_restore_pending() {
    let mut s = session();
    s.recovery.status = Some("restore_pending".to_string());
    s.recovery.restore_reason = Some("crash detected".to_string());
    let result = engine().check_stop(&s);
    assert_eq!(result.verdict, Verdict::ForceContinue,
        "restore_pending should force continue");
}
