// benches/find_symbol.rs
//
// Criterion benchmarks for sy_find_symbol with and without index.json.
// Run: cargo bench --bench find_symbol
//
// Measures the performance difference between:
//   - Full scan: no index, searches all N files
//   - Index-accelerated: .seeyue/index.json present, narrows to candidate files

use std::fs;
use std::sync::{Arc, Mutex};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tokio::sync::RwLock;

use seeyue_mcp::app_state::AppState;
use seeyue_mcp::tools::find_symbol::{run_find_symbol, FindSymbolParams};
use seeyue_mcp::tools::project_index::ProjectIndex;

fn make_state(workspace: std::path::PathBuf) -> AppState {
    AppState {
        workspace:      Arc::new(workspace.clone()),
        cache:          Arc::new(RwLock::new(seeyue_mcp::storage::cache::ReadCache::new())),
        checkpoint:     Arc::new(
            seeyue_mcp::storage::checkpoint::CheckpointStore::open(
                "bench", &workspace.join(".seeyue")
            ).unwrap()
        ),
        backup:         Arc::new(seeyue_mcp::storage::backup::BackupManager::new(
            seeyue_mcp::storage::backup::BackupConfig::default(), "bench".into()
        )),
        workflow_dir:   workspace.join("workflow"),
        policy_engine:  Arc::new(seeyue_mcp::policy::evaluator::PolicyEngine::new(
            seeyue_mcp::policy::spec_loader::PolicySpecs::load_empty()
        )),
        lsp_pool:       Arc::new(Mutex::new(seeyue_mcp::lsp::LspSessionPool::new())),
        skill_registry: Arc::new(seeyue_mcp::prompts::SkillRegistry::load_empty(&workspace)),
    }
}

/// Create a workspace with `n` source files, only one contains `target_fn`.
fn setup_workspace(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    for i in 0..n - 1 {
        fs::write(
            src.join(format!("mod{}.rs", i)),
            format!("pub fn noise_{}() {{}}\n", i),
        ).unwrap();
    }
    fs::write(src.join("target.rs"), "pub fn bench_target_fn() {}\n").unwrap();
    dir
}

fn bench_find_symbol(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let file_counts = [20usize, 50, 100];

    let mut group = c.benchmark_group("find_symbol");
    group.sample_size(10);

    for &n in &file_counts {
        // ── Without index (full scan) ──
        group.bench_with_input(
            BenchmarkId::new("full_scan", n),
            &n,
            |b, &n| {
                b.iter(|| {
                    let dir = setup_workspace(n);
                    let state = make_state(dir.path().to_path_buf());
                    rt.block_on(run_find_symbol(
                        FindSymbolParams {
                            name_path_pattern: "bench_target_fn".into(),
                            relative_path:     None,
                            substring_matching: Some(false),
                            include_body:       Some(false),
                            depth:              Some(1),
                        },
                        &state,
                    )).unwrap();
                });
            },
        );

        // ── With index ──
        group.bench_with_input(
            BenchmarkId::new("index_accelerated", n),
            &n,
            |b, &n| {
                b.iter(|| {
                    let dir = setup_workspace(n);
                    ProjectIndex::build(dir.path()).unwrap();
                    let state = make_state(dir.path().to_path_buf());
                    rt.block_on(run_find_symbol(
                        FindSymbolParams {
                            name_path_pattern: "bench_target_fn".into(),
                            relative_path:     None,
                            substring_matching: Some(false),
                            include_body:       Some(false),
                            depth:              Some(1),
                        },
                        &state,
                    )).unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_find_symbol);
criterion_main!(benches);
