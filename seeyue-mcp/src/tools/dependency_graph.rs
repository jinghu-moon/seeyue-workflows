// src/tools/dependency_graph.rs
//
// File-level dependency graph: who imports whom.
// Strategy: parse import/use/require statements via regex (no LSP needed).
// Supports: Rust (use/mod), TypeScript/JS (import/require), Python (import/from).
// Returns directed graph with impact_count for each node.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

use crate::error::ToolError;
use crate::lsp;

// ─── Params / Result ─────────────────────────────────────────────────────────

pub struct DependencyGraphParams {
    pub path:      String,
    pub depth:     Option<usize>,
    pub direction: Option<String>,  // "imports" | "imported_by" | "both"
}

#[derive(Debug, Serialize)]
pub struct DependencyGraphResult {
    pub status:       String,  // "ok" | "LSP_NOT_AVAILABLE" (informational)
    pub root:         String,
    pub direction:    String,
    pub nodes:        Vec<GraphNode>,
    pub edges:        Vec<GraphEdge>,
    pub total_nodes:  usize,
    pub total_edges:  usize,
    pub source:       String,  // "static_analysis"
}

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub path:         String,
    pub language:     String,
    pub impact_count: usize,  // number of files that import this file
    pub depth:        usize,
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub from: String,  // importer
    pub to:   String,  // importee
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_dependency_graph(
    params: DependencyGraphParams,
    workspace: &Path,
) -> Result<DependencyGraphResult, ToolError> {
    let root_path = crate::platform::path::resolve(workspace, &params.path)
        .map_err(|e| ToolError::PathEscape {
            file_path: params.path.clone(),
            hint: format!("{:?}", e),
        })?;

    if !root_path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.path.clone(),
            hint: "File does not exist.".to_string(),
        });
    }

    let max_depth = params.depth.unwrap_or(2).min(5);
    let direction = params.direction.as_deref().unwrap_or("imports");

    // Build full import map for workspace
    let import_map = build_import_map(workspace);

    let root_rel = to_rel(&root_path, workspace);

    let (nodes, edges) = match direction {
        "imported_by" => traverse_imported_by(&root_rel, &import_map, max_depth),
        "both" => {
            let (mut n1, mut e1) = traverse_imports(&root_rel, &import_map, max_depth);
            let (n2, e2) = traverse_imported_by(&root_rel, &import_map, max_depth);
            // Merge, dedup
            let existing_paths: HashSet<String> = n1.iter().map(|n| n.path.clone()).collect();
            for n in n2 { if !existing_paths.contains(&n.path) { n1.push(n); } }
            e1.extend(e2);
            (n1, e1)
        }
        _ => traverse_imports(&root_rel, &import_map, max_depth), // "imports"
    };

    // Compute impact_count: how many files import each node
    let mut impact: HashMap<String, usize> = HashMap::new();
    for edge in &edges {
        *impact.entry(edge.to.clone()).or_insert(0) += 1;
    }
    let nodes: Vec<GraphNode> = nodes.into_iter().map(|mut n| {
        n.impact_count = *impact.get(&n.path).unwrap_or(&0);
        n
    }).collect();

    let total_nodes = nodes.len();
    let total_edges = edges.len();

    Ok(DependencyGraphResult {
        status:      "ok".to_string(),
        root:        root_rel,
        direction:   direction.to_string(),
        nodes,
        edges,
        total_nodes,
        total_edges,
        source:      "static_analysis".to_string(),
    })
}

// ─── Import Map Builder ──────────────────────────────────────────────────────

/// Build a map: file_rel_path → set of imported file_rel_paths
fn build_import_map(workspace: &Path) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();

    let supported = ["rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py"];

    for entry in WalkDir::new(workspace)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target" && name != "node_modules"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !supported.contains(&ext) { continue; }

        let rel = to_rel(path, workspace);
        let imports = extract_imports(path, workspace);
        map.entry(rel).or_default().extend(imports);
    }

    map
}

/// Extract imported file paths from a source file (static analysis).
fn extract_imports(path: &Path, workspace: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else { return Vec::new(); };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let dir = path.parent().unwrap_or(workspace);

    match ext {
        "rs" => extract_rust_imports(&content, dir, workspace),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => extract_js_imports(&content, dir, workspace),
        "py" => extract_python_imports(&content, workspace),
        _ => Vec::new(),
    }
}

fn extract_js_imports(content: &str, dir: &Path, workspace: &Path) -> Vec<String> {
    // Match: import ... from './path'  or  require('./path')
    let re = Regex::new(r#"(?:import\s+[^'"]+\s+from\s+|require\s*\(\s*)['"](\.[^'"]+)['"]"#).unwrap();
    let mut results = Vec::new();
    for cap in re.captures_iter(content) {
        let raw = &cap[1];
        let resolved = resolve_import(raw, dir, workspace, &["ts", "tsx", "js", "jsx", "mjs"]);
        if let Some(r) = resolved { results.push(r); }
    }
    results
}

fn extract_rust_imports(content: &str, dir: &Path, workspace: &Path) -> Vec<String> {
    // Match: mod foo;  (looks for src/foo.rs or foo/mod.rs relative to file)
    // Use multiline mode so ^ matches start of each line
    let re = Regex::new(r"(?m)^\s*(?:pub\s+)?mod\s+(\w+)\s*;").unwrap();
    let mut results = Vec::new();
    for cap in re.captures_iter(content) {
        let modname = &cap[1];
        // Try dir/modname.rs first, then dir/modname/mod.rs
        let candidate1 = dir.join(format!("{}.rs", modname));
        let candidate2 = dir.join(modname).join("mod.rs");
        for c in [candidate1, candidate2] {
            if c.exists() {
                results.push(to_rel(&c, workspace));
                break;
            }
        }
    }
    results
}

fn extract_python_imports(content: &str, workspace: &Path) -> Vec<String> {
    // Match: from .module import ...  (relative imports only for file resolution)
    let re = Regex::new(r"^from\s+\.([\w.]+)\s+import").unwrap();
    let mut results = Vec::new();
    for cap in re.captures_iter(content) {
        let module = cap[1].replace('.', "/");
        let candidate = workspace.join(format!("{}.py", module));
        if candidate.exists() {
            results.push(to_rel(&candidate, workspace));
        }
    }
    results
}

fn resolve_import(
    raw: &str,
    dir: &Path,
    workspace: &Path,
    extensions: &[&str],
) -> Option<String> {
    let base = dir.join(raw);
    // Try exact, then with extensions
    if base.exists() && base.is_file() {
        return Some(to_rel(&base, workspace));
    }
    for ext in extensions {
        let with_ext = PathBuf::from(format!("{}.{}", base.display(), ext));
        if with_ext.exists() {
            return Some(to_rel(&with_ext, workspace));
        }
    }
    // Try index file
    for ext in extensions {
        let index = base.join(format!("index.{}", ext));
        if index.exists() {
            return Some(to_rel(&index, workspace));
        }
    }
    None
}

// ─── Graph Traversal ─────────────────────────────────────────────────────────

/// BFS: follow imports outward from root.
fn traverse_imports(
    root: &str,
    map: &HashMap<String, HashSet<String>>,
    max_depth: usize,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut visited: HashSet<String> = HashSet::new();
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    visited.insert(root.to_string());
    nodes.push(GraphNode { path: root.to_string(), language: detect_lang(root), impact_count: 0, depth: 0 });
    queue.push_back((root.to_string(), 0));

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth { continue; }
        let empty = HashSet::new();
        let imports = map.get(&current).unwrap_or(&empty);
        for imp in imports {
            edges.push(GraphEdge { from: current.clone(), to: imp.clone() });
            if !visited.contains(imp) {
                visited.insert(imp.clone());
                nodes.push(GraphNode { path: imp.clone(), language: detect_lang(imp), impact_count: 0, depth: depth + 1 });
                queue.push_back((imp.clone(), depth + 1));
            }
        }
    }
    (nodes, edges)
}

/// BFS: find files that import root (reverse direction).
fn traverse_imported_by(
    root: &str,
    map: &HashMap<String, HashSet<String>>,
    max_depth: usize,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    // Build reverse map
    let mut reverse: HashMap<String, HashSet<String>> = HashMap::new();
    for (file, imports) in map {
        for imp in imports {
            reverse.entry(imp.clone()).or_default().insert(file.clone());
        }
    }
    traverse_imports(root, &reverse, max_depth)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn to_rel(path: &Path, workspace: &Path) -> String {
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn detect_lang(path: &str) -> String {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    lsp::language_id(ext).to_string()
}
