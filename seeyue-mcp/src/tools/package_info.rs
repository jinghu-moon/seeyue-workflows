// src/tools/package_info.rs
//
// Query package registries for latest version and metadata.
// Supports: crates.io (Rust), npm (Node), PyPI (Python).
// In-memory cache with TTL=1h per (registry, name) key.
// Uses async reqwest — DNS timeout is handled by tokio::time::timeout at call site.
// Network unavailable → caller receives NETWORK_ERROR result.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::Serialize;

use crate::error::ToolError;

// ─── In-memory cache ─────────────────────────────────────────────────────────

struct CacheEntry {
    result:     PackageInfoResult,
    fetched_at: Instant,
}

static CACHE: Lazy<Mutex<HashMap<String, CacheEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const CACHE_TTL: Duration = Duration::from_secs(3600);

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct PackageInfoParams {
    pub name:     String,
    pub registry: Option<String>,  // "crates" | "npm" | "pypi"
    pub version:  Option<String>,  // specific version, default latest
}

#[derive(Debug, Serialize, Clone)]
pub struct PackageInfoResult {
    pub status:      String,   // "ok" | "NOT_FOUND" | "NETWORK_ERROR"
    pub registry:    String,
    pub name:        String,
    pub version:     String,
    pub description: Option<String>,
    pub homepage:    Option<String>,
    pub cached:      bool,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub async fn run_package_info(
    params: PackageInfoParams,
) -> Result<PackageInfoResult, ToolError> {
    let registry = detect_registry(&params.name, params.registry.as_deref());
    let cache_key = format!("{}:{}", registry, params.name);

    // Check cache
    {
        let cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = cache.get(&cache_key) {
            if entry.fetched_at.elapsed() < CACHE_TTL {
                let mut cached = entry.result.clone();
                cached.cached = true;
                return Ok(cached);
            }
        }
    }

    let result = fetch_package(&params.name, &registry, params.version.as_deref()).await?;

    // Store in cache
    {
        let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
        cache.insert(cache_key, CacheEntry {
            result:     result.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(result)
}

// ─── Registry Detection ──────────────────────────────────────────────────────

fn detect_registry(name: &str, hint: Option<&str>) -> String {
    if let Some(h) = hint {
        return h.to_lowercase();
    }
    // Heuristic: @scope/pkg → npm
    if name.starts_with('@') || name.contains('/') {
        return "npm".to_string();
    }
    "crates".to_string()
}

// ─── Async Fetchers ──────────────────────────────────────────────────────────

async fn fetch_package(
    name:     &str,
    registry: &str,
    _version: Option<&str>,
) -> Result<PackageInfoResult, ToolError> {
    match registry {
        "npm"  => fetch_npm(name).await,
        "pypi" => fetch_pypi(name).await,
        _      => fetch_crates(name).await,
    }
}

fn build_client() -> Result<reqwest::Client, ToolError> {
    reqwest::Client::builder()
        .user_agent("seeyue-mcp/0.1 (github.com/seeyue)")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })
}

async fn fetch_crates(name: &str) -> Result<PackageInfoResult, ToolError> {
    let client = build_client()?;
    let url = format!("https://crates.io/api/v1/crates/{}", name);

    let resp = client.get(&url).send().await
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })?;

    if resp.status().as_u16() == 404 {
        return Ok(not_found("crates", name));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })?;

    Ok(PackageInfoResult {
        status:      "ok".to_string(),
        registry:    "crates".to_string(),
        name:        name.to_string(),
        version:     json["crate"]["newest_version"].as_str().unwrap_or("unknown").to_string(),
        description: json["crate"]["description"].as_str().map(str::to_string),
        homepage:    json["crate"]["homepage"].as_str()
                        .or_else(|| json["crate"]["repository"].as_str())
                        .map(str::to_string),
        cached:      false,
    })
}

async fn fetch_npm(name: &str) -> Result<PackageInfoResult, ToolError> {
    let client = build_client()?;
    let url = format!("https://registry.npmjs.org/{}", name);

    let resp = client.get(&url).send().await
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })?;

    if resp.status().as_u16() == 404 {
        return Ok(not_found("npm", name));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })?;

    Ok(PackageInfoResult {
        status:      "ok".to_string(),
        registry:    "npm".to_string(),
        name:        name.to_string(),
        version:     json["dist-tags"]["latest"].as_str().unwrap_or("unknown").to_string(),
        description: json["description"].as_str().map(str::to_string),
        homepage:    json["homepage"].as_str().map(str::to_string),
        cached:      false,
    })
}

async fn fetch_pypi(name: &str) -> Result<PackageInfoResult, ToolError> {
    let client = build_client()?;
    let url = format!("https://pypi.org/pypi/{}/json", name);

    let resp = client.get(&url).send().await
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })?;

    if resp.status().as_u16() == 404 {
        return Ok(not_found("pypi", name));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| ToolError::IoError { message: format!("NETWORK_ERROR: {}", e) })?;

    Ok(PackageInfoResult {
        status:      "ok".to_string(),
        registry:    "pypi".to_string(),
        name:        name.to_string(),
        version:     json["info"]["version"].as_str().unwrap_or("unknown").to_string(),
        description: json["info"]["summary"].as_str().map(str::to_string),
        homepage:    json["info"]["home_page"].as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
        cached:      false,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn not_found(registry: &str, name: &str) -> PackageInfoResult {
    PackageInfoResult {
        status:      "NOT_FOUND".to_string(),
        registry:    registry.to_string(),
        name:        name.to_string(),
        version:     String::new(),
        description: None,
        homepage:    None,
        cached:      false,
    }
}
