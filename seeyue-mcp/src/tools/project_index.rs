// src/tools/project_index.rs
//
// ProjectIndex: persistent symbol index snapshot at .seeyue/index.json.
// Format follows gap-analysis §A4.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ToolError;
use crate::treesitter::languages::detect_language;
use crate::treesitter::symbols::extract_ts_symbols;

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub name_path: String,
    pub kind:      String,
    pub line:      usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub mtime:   u64,
    pub symbols: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub generated_at:   String,
    pub workspace_root: String,
    pub files:          HashMap<String, FileEntry>,
}

impl ProjectIndex {
    /// Build a full index from scratch and write to .seeyue/index.json.
    pub fn build(workspace: &Path) -> Result<Self, ToolError> {
        let mut files = HashMap::new();
        collect_files(workspace, workspace, &mut files);
        let idx = ProjectIndex {
            generated_at:   now_rfc3339(),
            workspace_root: workspace.to_string_lossy().to_string(),
            files,
        };
        idx.save(workspace)?;
        Ok(idx)
    }

    /// Load an existing index from .seeyue/index.json.
    /// Returns an empty index (no error) if the file does not exist.
    pub fn load(workspace: &Path) -> Result<Self, ToolError> {
        let path = index_path(workspace);
        if !path.exists() {
            return Ok(ProjectIndex {
                generated_at:   now_rfc3339(),
                workspace_root: workspace.to_string_lossy().to_string(),
                files:          HashMap::new(),
            });
        }
        let text = std::fs::read_to_string(&path).map_err(|e| ToolError::IoError {
            message: format!("Failed to read index.json: {e}"),
        })?;
        serde_json::from_str(&text).map_err(|e| ToolError::IoError {
            message: format!("Failed to parse index.json: {e}"),
        })
    }

    /// Incrementally update: only rebuild files whose mtime has changed.
    pub fn update(workspace: &Path) -> Result<Self, ToolError> {
        let mut idx = ProjectIndex::load(workspace)?;
        idx.generated_at = now_rfc3339();
        idx.workspace_root = workspace.to_string_lossy().to_string();

        // Discover all current source files
        let mut current: HashMap<String, u64> = HashMap::new();
        collect_mtimes(workspace, workspace, &mut current);

        // Remove entries for deleted files
        idx.files.retain(|k, _| current.contains_key(k));

        // Rebuild entries for new or modified files
        for (rel, mtime) in &current {
            let needs_rebuild = idx.files
                .get(rel)
                .map(|e| e.mtime != *mtime)
                .unwrap_or(true);
            if needs_rebuild {
                let abs = workspace.join(rel);
                if let Ok(content) = std::fs::read_to_string(&abs) {
                    let lang = detect_language(&abs);
                    let syms = extract_ts_symbols(&content, &lang);
                    let symbols = flatten_symbols(&syms, None);
                    idx.files.insert(rel.clone(), FileEntry { mtime: *mtime, symbols });
                }
            }
        }

        idx.save(workspace)?;
        Ok(idx)
    }

    /// Query the index for entries matching name_path (exact or substring).
    pub fn query(&self, pattern: &str, substring: bool) -> Vec<&IndexEntry> {
        let mut out = Vec::new();
        for entry in self.files.values() {
            for sym in &entry.symbols {
                let matches = if substring {
                    sym.name_path.contains(pattern)
                } else {
                    sym.name_path == pattern
                        || sym.name_path.ends_with(&format!("/{}", pattern))
                        || sym.name_path == pattern
                };
                if matches {
                    out.push(sym);
                }
            }
        }
        out
    }

    /// Write the index to disk using atomic write (.tmp → rename).
    fn save(&self, workspace: &Path) -> Result<(), ToolError> {
        let dir = workspace.join(".seeyue");
        std::fs::create_dir_all(&dir).map_err(|e| ToolError::IoError {
            message: format!("Failed to create .seeyue dir: {e}"),
        })?;
        let target = dir.join("index.json");
        let tmp    = dir.join("index.json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(|e| ToolError::IoError {
            message: format!("Failed to serialize index: {e}"),
        })?;
        std::fs::write(&tmp, text).map_err(|e| ToolError::IoError {
            message: format!("Failed to write index.tmp: {e}"),
        })?;
        std::fs::rename(&tmp, &target).map_err(|e| ToolError::IoError {
            message: format!("Failed to rename index.tmp -> index.json: {e}"),
        })?;
        Ok(())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn index_path(workspace: &Path) -> PathBuf {
    workspace.join(".seeyue/index.json")
}

fn now_rfc3339() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO-8601 approximation (no chrono dependency)
    format!("{}Z", secs)
}

fn file_mtime(path: &Path) -> u64 {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0)
}

fn collect_files(root: &Path, dir: &Path, out: &mut HashMap<String, FileEntry>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e)  => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_files(root, &path, out);
        } else if is_source_file(&path) {
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let mtime   = file_mtime(&path);
                let lang    = detect_language(&path);
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let syms    = extract_ts_symbols(&content, &lang);
                let symbols = flatten_symbols(&syms, None);
                out.insert(rel_str, FileEntry { mtime, symbols });
            }
        }
    }
}

fn collect_mtimes(root: &Path, dir: &Path, out: &mut HashMap<String, u64>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e)  => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_mtimes(root, &path, out);
        } else if is_source_file(&path) {
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                out.insert(rel_str, file_mtime(&path));
            }
        }
    }
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "rs" | "py" | "ts" | "tsx" | "js" | "go" | "java" | "c" | "cpp" | "cs"
    )
}

fn flatten_symbols(
    syms: &[crate::treesitter::symbols::TsSymbol],
    parent: Option<&str>,
) -> Vec<IndexEntry> {
    let mut out = Vec::new();
    for sym in syms {
        let np = match parent {
            Some(p) => format!("{}/{}", p, sym.name),
            None    => sym.name.clone(),
        };
        out.push(IndexEntry {
            name_path: np.clone(),
            kind:      sym.kind.clone(),
            line:      sym.start_line,
        });
        if !sym.children.is_empty() {
            let mut children = flatten_symbols(&sym.children, Some(&np));
            out.append(&mut children);
        }
    }
    out
}
