// tests/bench_policy.rs
//
// P1.5 Performance micro-benchmarks for the policy engine.
// Measures pure Rust decision latency (no MCP/stdio overhead).
//
// Run (release, authoritative):
//   cargo test --release --test bench_policy -- --nocapture
//
// Run (debug, smoke-test only):
//   cargo test --test bench_policy -- --nocapture
//
// Release target: all decision operations < 1ms (< 100μs typical).
// Debug target:   relaxed 20x (regex unoptimized); smoke-test only.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use seeyue_mcp::policy::command;
use seeyue_mcp::policy::evaluator::PolicyEngine;
use seeyue_mcp::policy::file_class;
use seeyue_mcp::policy::spec_loader::PolicySpecs;
use seeyue_mcp::workflow::state::{
    SessionState, PhaseState, NodeState, LoopBudget, ApprovalState, RecoveryState,
};

// ─── Threshold multiplier ───────────────────────────────────────────────────
//
// Debug builds have unoptimized regex (~12-20x slower). We relax thresholds
// so `cargo test` passes in debug, while `cargo test --release` enforces the
// real < 1ms target.

#[cfg(debug_assertions)]
const THRESHOLD_MULTIPLIER: u64 = 20;

#[cfg(not(debug_assertions))]
const THRESHOLD_MULTIPLIER: u64 = 1;

/// Build a Duration threshold: base_ms * THRESHOLD_MULTIPLIER.
fn threshold_ms(base_ms: u64) -> Duration {
    Duration::from_millis(base_ms * THRESHOLD_MULTIPLIER)
}

fn threshold_label() -> &'static str {
    if cfg!(debug_assertions) { "DEBUG (20x relaxed)" } else { "RELEASE" }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn project_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().to_path_buf()
}

fn load_specs() -> PolicySpecs {
    PolicySpecs::load(&project_root())
        .unwrap_or_else(|_| PolicySpecs::load_empty())
}

fn make_session() -> SessionState {
    SessionState {
        schema_version: Some(1),
        run_id: Some("wf-bench-001".to_string()),
        phase: PhaseState {
            id: Some("P1".to_string()),
            name: Some("Implementation".to_string()),
            status: Some("active".to_string()),
        },
        node: NodeState {
            id: Some("P1-N1".to_string()),
            name: Some("Test Node".to_string()),
            status: Some("active".to_string()),
            state: Some("green_verified".to_string()),
            tdd_required: Some(false),
            tdd_state: None,
            target: Some(vec!["src/".to_string()]),
            test_contract: None,
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
        },
    }
}

struct BenchResult {
    name: String,
    iterations: usize,
    total: Duration,
    min: Duration,
    max: Duration,
    p50: Duration,
    p95: Duration,
    p99: Duration,
}

impl BenchResult {
    fn avg(&self) -> Duration {
        self.total / self.iterations as u32
    }
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<45} {:>6} iters | avg {:>8.1}\u{03bc}s | min {:>7.1}\u{03bc}s | p50 {:>7.1}\u{03bc}s | p95 {:>7.1}\u{03bc}s | p99 {:>7.1}\u{03bc}s | max {:>7.1}\u{03bc}s",
            self.name,
            self.iterations,
            self.avg().as_nanos() as f64 / 1000.0,
            self.min.as_nanos() as f64 / 1000.0,
            self.p50.as_nanos() as f64 / 1000.0,
            self.p95.as_nanos() as f64 / 1000.0,
            self.p99.as_nanos() as f64 / 1000.0,
            self.max.as_nanos() as f64 / 1000.0,
        )
    }
}

fn bench<F: FnMut()>(name: &str, iterations: usize, mut f: F) -> BenchResult {
    // Warmup
    for _ in 0..100 {
        f();
    }

    let mut durations = Vec::with_capacity(iterations);
    let start_total = Instant::now();

    for _ in 0..iterations {
        let start = Instant::now();
        f();
        durations.push(start.elapsed());
    }

    let total = start_total.elapsed();
    durations.sort();

    let p = |pct: usize| -> Duration {
        let idx = (pct * durations.len() / 100).min(durations.len() - 1);
        durations[idx]
    };

    BenchResult {
        name: name.to_string(),
        iterations,
        total,
        min: durations[0],
        max: *durations.last().unwrap(),
        p50: p(50),
        p95: p(95),
        p99: p(99),
    }
}

// ─── Benchmarks ──────────────────────────────────────────────────────────────

const ITERS: usize = 10_000;

#[test]
fn bench_spec_loading() {
    let sep = "=".repeat(120);
    println!("\n{sep}");
    println!("  P1.5 Policy Engine -- Spec Loading (cold start)  [{}]", threshold_label());
    println!("{sep}\n");

    let r = bench("PolicySpecs::load() [cold, from YAML]", 100, || {
        let _ = PolicySpecs::load(&project_root());
    });
    println!("  {r}");

    let limit = threshold_ms(50);
    assert!(
        r.p99 < limit,
        "Spec loading p99 ({:?}) should be < {:?}",
        r.p99, limit
    );
}

#[test]
fn bench_command_classification() {
    let specs = load_specs();

    let sep = "=".repeat(120);
    println!("\n{sep}");
    println!("  P1.5 Policy Engine -- Command Classification  [{}]", threshold_label());
    println!("{sep}\n");

    let commands = [
        ("ls -la",                       "safe"),
        ("echo hello",                   "safe"),
        ("cat README.md",               "safe"),
        ("cargo test",                   "verify"),
        ("npm test",                     "verify"),
        ("pytest -v",                    "verify"),
        ("rm -rf /",                     "destructive"),
        ("del /s /q *",                  "destructive"),
        ("git commit -m 'test'",         "git_mutating"),
        ("git push origin main",         "git_mutating"),
        ("curl https://example.com",     "network_sensitive"),
        ("wget http://example.com/f",    "network_sensitive"),
        ("sudo chmod 777 /etc/passwd",   "privileged"),
        ("npm install express",          "network_sensitive"),
    ];

    let limit = threshold_ms(1);
    let mut all_pass = true;

    for (cmd, _expected) in &commands {
        let r = bench(
            &format!("classify_command({:30})", format!("\"{}\"", cmd)),
            ITERS,
            || {
                let _ = command::classify_command(cmd, &specs);
            },
        );
        println!("  {r}");

        if r.p99 >= limit {
            eprintln!("  WARNING: p99 {:?} >= {:?} for '{cmd}'", r.p99, limit);
            all_pass = false;
        }
    }

    // Aggregate: classify all commands in a single call
    // Note: this is 14 sequential classifies — not a realistic single-request scenario.
    // The important metric is per-command p99 (checked above).
    let r = bench("classify_command() [all 14 commands]", ITERS, || {
        for (cmd, _) in &commands {
            let _ = command::classify_command(cmd, &specs);
        }
    });
    println!("\n  {r}");

    assert!(all_pass, "Some individual command classifications exceeded p99 {:?}", limit);
}

#[test]
fn bench_file_classification() {
    let specs = load_specs();

    let sep = "=".repeat(120);
    println!("\n{sep}");
    println!("  P1.5 Policy Engine -- File Classification  [{}]", threshold_label());
    println!("{sep}\n");

    let paths = [
        "src/main.rs",
        "src/policy/evaluator.rs",
        "docs/architecture-v4.md",
        "tests/test_policy.py",
        ".env",
        ".env.local",
        "secrets/api-key.pem",
        "workflow/policy.spec.yaml",
        ".github/workflows/ci.yml",
        ".claude/settings.json",
        "dist/bundle.js",
    ];

    let limit = threshold_ms(1);
    let mut all_pass = true;

    for path in &paths {
        let r = bench(
            &format!("classify_file({:30})", format!("\"{}\"", path)),
            ITERS,
            || {
                let _ = file_class::classify_file(path, &specs);
            },
        );
        println!("  {r}");

        if r.p99 >= limit {
            eprintln!("  WARNING: p99 {:?} >= {:?} for '{path}'", r.p99, limit);
            all_pass = false;
        }
    }

    // Aggregate
    let r = bench("classify_file() [all 11 paths]", ITERS, || {
        for path in &paths {
            let _ = file_class::classify_file(path, &specs);
        }
    });
    println!("\n  {r}");

    assert!(all_pass, "Some file classifications exceeded p99 {:?}", limit);
    assert!(
        r.p99 < limit,
        "Aggregate p99 ({:?}) should be < {:?} for 11 paths",
        r.p99, limit
    );
}

#[test]
fn bench_policy_evaluator() {
    let specs = load_specs();
    let engine = PolicyEngine::new(specs);
    let session = make_session();

    let sep = "=".repeat(120);
    println!("\n{sep}");
    println!("  P1.5 Policy Engine -- Full Policy Evaluation  [{}]", threshold_label());
    println!("{sep}\n");

    let limit = threshold_ms(1);

    // check_bash
    let bash_cases = [
        "ls -la",
        "rm -rf /",
        "git push origin main",
        "cargo test --release",
        "curl https://api.example.com/data",
        "sudo systemctl restart nginx",
    ];

    for cmd in &bash_cases {
        let r = bench(
            &format!("check_bash({:35})", format!("\"{}\"", cmd)),
            ITERS,
            || {
                let _ = engine.check_bash(cmd, &session);
            },
        );
        println!("  {r}");
        assert!(
            r.p99 < limit,
            "check_bash('{}') p99 ({:?}) should be < {:?}",
            cmd,
            r.p99,
            limit
        );
    }

    println!();

    // check_write
    let write_cases = [
        "src/main.rs",
        ".env",
        "workflow/policy.spec.yaml",
        "tests/test_policy.py",
        "docs/README.md",
        ".github/workflows/ci.yml",
    ];

    for path in &write_cases {
        let r = bench(
            &format!("check_write({:35})", format!("\"{}\"", path)),
            ITERS,
            || {
                let _ = engine.check_write(path, &session);
            },
        );
        println!("  {r}");
        assert!(
            r.p99 < limit,
            "check_write('{}') p99 ({:?}) should be < {:?}",
            path,
            r.p99,
            limit
        );
    }

    println!();

    // check_stop
    let r = bench("check_stop()", ITERS, || {
        let _ = engine.check_stop(&session);
    });
    println!("  {r}");
    assert!(
        r.p99 < limit,
        "check_stop() p99 ({:?}) should be < {:?}",
        r.p99,
        limit
    );
}

#[test]
fn bench_session_loading() {
    let sep = "=".repeat(120);
    println!("\n{sep}");
    println!("  P1.5 Policy Engine -- Session YAML Parsing  [{}]", threshold_label());
    println!("{sep}\n");

    let tmp = tempfile::tempdir().unwrap();
    let workflow_dir = tmp.path().to_path_buf();

    std::fs::write(
        workflow_dir.join("session.yaml"),
        "schema_version: 1\n\
         run_id: wf-bench-001\n\
         phase:\n\
         \x20 id: P1\n\
         \x20 name: Implementation\n\
         \x20 status: active\n\
         node:\n\
         \x20 id: P1-N1\n\
         \x20 name: Bench Node\n\
         \x20 status: active\n\
         \x20 state: green_verified\n\
         \x20 tdd_required: false\n\
         loop_budget:\n\
         \x20 max: 100\n\
         \x20 used: 5\n\
         \x20 exhausted: false\n\
         approvals:\n\
         \x20 pending: []\n\
         \x20 grants: []\n\
         recovery:\n\
         \x20 status: null\n",
    )
    .unwrap();

    let limit = threshold_ms(1);
    let r = bench("load_session() [from YAML file]", ITERS, || {
        let _ = seeyue_mcp::workflow::state::load_session(&workflow_dir);
    });
    println!("  {r}");

    assert!(
        r.p99 < limit,
        "load_session() p99 ({:?}) should be < {:?}",
        r.p99, limit
    );
}

#[test]
fn bench_summary() {
    let specs = load_specs();
    let engine = PolicyEngine::new(specs);
    let session = make_session();

    let sep = "=".repeat(120);
    println!("\n{sep}");
    println!("  P1.5 Summary -- Worst-Case Full Decision Pipeline  [{}]", threshold_label());
    println!("{sep}\n");

    // Worst case: destructive command
    let r1 = bench(
        "WORST CASE: check_bash(\"rm -rf /\")",
        ITERS * 10,
        || {
            let _ = engine.check_bash("rm -rf /", &session);
        },
    );
    println!("  {r1}");

    // Worst case: secret file
    let r2 = bench(
        "WORST CASE: check_write(\".env\")",
        ITERS * 10,
        || {
            let _ = engine.check_write(".env", &session);
        },
    );
    println!("  {r2}");

    // Happy path: safe command
    let r3 = bench(
        "HAPPY PATH: check_bash(\"ls\")",
        ITERS * 10,
        || {
            let _ = engine.check_bash("ls", &session);
        },
    );
    println!("  {r3}");

    let worst_p99 = [r1.p99, r2.p99, r3.p99].into_iter().max().unwrap();
    let limit = threshold_ms(1);
    let pass = worst_p99 < limit;

    println!("\n  Target: all p99 < {:?} ({})", limit, threshold_label());
    println!(
        "  Result: worst p99 = {:.1}us -> {}",
        worst_p99.as_nanos() as f64 / 1000.0,
        if pass { "PASS" } else { "FAIL" }
    );
    println!("{sep}\n");

    assert!(pass, "Worst-case p99 ({:?}) exceeds {:?} target", worst_p99, limit);
}
