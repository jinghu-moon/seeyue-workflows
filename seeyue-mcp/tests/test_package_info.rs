// tests/test_package_info.rs
//
// Tests for tools::package_info::run_package_info.
// Run: cargo test --test test_package_info
// Note: tests are network-tolerant — NETWORK_ERROR is an acceptable outcome.

use seeyue_mcp::tools::package_info::{PackageInfoParams, run_package_info};

fn params(name: &str, registry: Option<&str>) -> PackageInfoParams {
    PackageInfoParams {
        name:     name.into(),
        registry: registry.map(|s| s.into()),
        version:  None,
    }
}

#[tokio::test]
async fn test_crates_registry_result_has_name() {
    let result = run_package_info(params("serde", Some("crates"))).await.unwrap();
    assert_eq!(result.name, "serde");
    assert_eq!(result.registry, "crates");
}

#[tokio::test]
async fn test_npm_registry_result_has_name() {
    let result = match run_package_info(params("lodash", Some("npm"))).await {
        Ok(r) => r,
        Err(_) => return, // network-tolerant
    };
    if result.status == "NETWORK_ERROR" { return; }
    assert_eq!(result.name, "lodash");
    assert_eq!(result.registry, "npm");
}

#[tokio::test]
async fn test_pypi_registry_result_has_name() {
    let result = match run_package_info(params("requests", Some("pypi"))).await {
        Ok(r) => r,
        Err(_) => return, // network-tolerant
    };
    if result.status == "NETWORK_ERROR" { return; }
    assert_eq!(result.name, "requests");
    assert_eq!(result.registry, "pypi");
}

#[tokio::test]
async fn test_status_is_ok_or_network_error() {
    let result = run_package_info(params("serde", Some("crates"))).await.unwrap();
    assert!(
        result.status == "ok" || result.status == "NETWORK_ERROR" || result.status == "NOT_FOUND",
        "unexpected status: {}", result.status
    );
}

#[tokio::test]
async fn test_not_found_package() {
    let result = run_package_info(params("zzz_this_package_does_not_exist_xyz_42", Some("crates"))).await.unwrap();
    assert!(
        result.status == "NOT_FOUND" || result.status == "NETWORK_ERROR",
        "expected NOT_FOUND or NETWORK_ERROR, got: {}", result.status
    );
}

#[tokio::test]
async fn test_cached_field_accessible() {
    let result = run_package_info(params("serde", Some("crates"))).await.unwrap();
    // cached is bool — just verify it's accessible
    let _ = result.cached;
}
