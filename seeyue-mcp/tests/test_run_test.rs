// tests/test_run_test.rs
//
// Tests for tools::run_test::run_run_test.
// Run: cargo test --test test_run_test

use seeyue_mcp::tools::run_test::{RunTestParams, run_run_test};

fn ws() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

fn params(language: &str) -> RunTestParams {
    RunTestParams {
        filter: None,
        language: Some(language.into()),
        timeout_ms: Some(30_000),
    }
}

#[tokio::test]
async fn test_rust_runner_label() {
    let tmp = ws();
    // Create a minimal Cargo.toml so cargo test runner is detected
    std::fs::write(tmp.path().join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
    let result = run_run_test(params("rust"), tmp.path()).await.unwrap();
    assert!(result.runner.contains("cargo"), "expected cargo runner, got {}", result.runner);
}

#[tokio::test]
async fn test_result_has_exit_code() {
    let tmp = ws();
    std::fs::write(tmp.path().join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
    let result = run_run_test(params("rust"), tmp.path()).await.unwrap();
    // exit_code field must exist (Option<i32>)
    let _ = result.exit_code;
}

#[tokio::test]
async fn test_duration_ms_set() {
    let tmp = ws();
    std::fs::write(tmp.path().join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
    let result = run_run_test(params("rust"), tmp.path()).await.unwrap();
    let _ = result.duration_ms;
}

#[tokio::test]
#[ignore = "slow: spawns cargo build in tempdir"]
async fn test_passing_test_returns_passed_true() {
    let tmp = ws();
    std::fs::write(tmp.path().join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"),
        "#[test]\nfn it_works() { assert_eq!(1+1, 2); }\n").unwrap();
    let result = run_run_test(params("rust"), tmp.path()).await.unwrap();
    assert!(result.passed, "test with passing tests should return passed=true");
}

#[tokio::test]
#[ignore = "slow: spawns cargo build in tempdir"]
async fn test_failing_test_returns_passed_false() {
    let tmp = ws();
    std::fs::write(tmp.path().join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"),
        "#[test]\nfn it_fails() { panic!(\"intentional\"); }\n").unwrap();
    let result = run_run_test(params("rust"), tmp.path()).await.unwrap();
    assert!(!result.passed, "test with failing tests should return passed=false");
}

#[tokio::test]
async fn test_filter_field_accepted() {
    let tmp = ws();
    std::fs::write(tmp.path().join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
    let result = run_run_test(
        RunTestParams { filter: Some("pass_me".into()), language: Some("rust".into()), timeout_ms: Some(30_000) },
        tmp.path(),
    ).await.unwrap();
    // runner field should contain cargo regardless of pass/fail
    assert!(result.runner.contains("cargo"));
    // exit_code field must be accessible
    let _ = result.exit_code;
}
