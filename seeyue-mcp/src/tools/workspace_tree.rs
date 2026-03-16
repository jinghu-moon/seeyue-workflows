use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{Duration, SystemTime};

use ignore::WalkBuilder;

use crate::error::ToolError;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct WorkspaceTreeParams {
    pub depth:              Option<usize>,
    pub respect_gitignore:  Option<bool>,
    pub show_hidden:        Option<bool>,
    pub min_size_bytes:     Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceTreeResult {
    #[serde(rename = "type")]
    pub kind:    String, // "success"
    pub root:    String,
    pub tree:    Vec<TreeNode>,
    pub summary: TreeSummary,
}

#[derive(Debug, Serialize)]
pub struct TreeSummary {
    pub total_files: usize,
    pub total_dirs:  usize,
    pub languages:   HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct TreeNode {
    pub name:  String,
    pub kind:  String, // "dir" | "file"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size:  Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_ago: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeNode>>,
}

#[derive(Debug, Default)]
struct Node {
    name:     String,
    is_dir:   bool,
    size:     Option<u64>,
    language: Option<String>,
    modified_ago: Option<String>,
    children: BTreeMap<String, Node>,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_workspace_tree(
    params: WorkspaceTreeParams,
    workspace: &Path,
) -> Result<WorkspaceTreeResult, ToolError> {
    let depth = params.depth.unwrap_or(3).max(1);
    let respect_gitignore = params.respect_gitignore.unwrap_or(true);
    let show_hidden = params.show_hidden.unwrap_or(false);
    let min_size = params.min_size_bytes.unwrap_or(0);

    let mut builder = WalkBuilder::new(workspace);
    builder.follow_links(false);
    builder.max_depth(Some(depth));
    builder.hidden(!show_hidden);
    builder.git_ignore(respect_gitignore);
    builder.git_global(respect_gitignore);
    builder.git_exclude(respect_gitignore);
    builder.ignore(respect_gitignore);
    builder.parents(respect_gitignore);

    let mut root = Node {
        name: workspace.to_string_lossy().to_string(),
        is_dir: true,
        ..Default::default()
    };

    let mut total_files = 0usize;
    let mut total_dirs = 0usize;
    let mut languages: HashMap<String, usize> = HashMap::new();

    for entry in builder.build().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path == workspace {
            continue;
        }

        let rel = match path.strip_prefix(workspace) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let components: Vec<_> = rel.components().collect();
        if components.is_empty() {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        let mut cursor = &mut root;
        for (idx, comp) in components.iter().enumerate() {
            let name = comp.as_os_str().to_string_lossy().to_string();
            let is_last = idx == components.len() - 1;

            if is_last {
                if is_dir {
                    let node = cursor.children.entry(name.clone()).or_insert_with(|| Node {
                        name: name.clone(),
                        is_dir: true,
                        ..Default::default()
                    });
                    node.is_dir = true;
                    total_dirs += 1;
                } else {
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    if meta.len() < min_size {
                        continue;
                    }
                    let language = detect_language(&name);
                    if let Some(lang) = &language {
                        *languages.entry(lang.clone()).or_insert(0) += 1;
                    }
                    let modified_ago = meta.modified().ok().and_then(format_ago);
                    cursor.children.insert(name.clone(), Node {
                        name: name.clone(),
                        is_dir: false,
                        size: Some(meta.len()),
                        language,
                        modified_ago,
                        children: BTreeMap::new(),
                    });
                    total_files += 1;
                }
            } else {
                cursor = cursor.children.entry(name.clone()).or_insert_with(|| Node {
                    name: name.clone(),
                    is_dir: true,
                    ..Default::default()
                });
            }
        }
    }

    let tree = root.children.values().map(to_tree_node).collect();

    Ok(WorkspaceTreeResult {
        kind: "success".into(),
        root: workspace.to_string_lossy().to_string(),
        tree,
        summary: TreeSummary {
            total_files,
            total_dirs,
            languages,
        },
    })
}

// ─── 转换与辅助 ───────────────────────────────────────────────────────────────

fn to_tree_node(node: &Node) -> TreeNode {
    let children = if node.is_dir {
        let kids: Vec<TreeNode> = node.children.values().map(to_tree_node).collect();
        if kids.is_empty() { None } else { Some(kids) }
    } else {
        None
    };

    TreeNode {
        name: node.name.clone(),
        kind: if node.is_dir { "dir".into() } else { "file".into() },
        size: node.size,
        language: node.language.clone(),
        modified_ago: node.modified_ago.clone(),
        children,
    }
}

fn detect_language(name: &str) -> Option<String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    Some(match ext.as_str() {
        "rs"                    => "rust",
        "py"|"pyw"              => "python",
        "ts"                    => "typescript",
        "tsx"                   => "tsx",
        "js"|"mjs"|"cjs"        => "javascript",
        "jsx"                   => "jsx",
        "go"                    => "go",
        "c"|"h"                 => "c",
        "cpp"|"cxx"|"cc"|"hpp"  => "c++",
        "java"                  => "java",
        "kt"|"kts"              => "kotlin",
        "rb"                    => "ruby",
        "swift"                 => "swift",
        "cs"                    => "c#",
        "sh"|"bash"             => "shell",
        "toml"                  => "toml",
        "json"                  => "json",
        "yaml"|"yml"            => "yaml",
        "md"                    => "markdown",
        "sql"                   => "sql",
        "html"                  => "html",
        "css"                   => "css",
        _                        => return None,
    }.to_string())
}

fn format_ago(time: SystemTime) -> Option<String> {
    let now = SystemTime::now();
    let diff = now.duration_since(time).ok().unwrap_or(Duration::from_secs(0));
    let secs = diff.as_secs();
    let text = if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else if secs < 86400 * 7 {
        format!("{}d", secs / 86400)
    } else if secs < 86400 * 30 {
        format!("{}w", secs / 604800)
    } else {
        format!("{}mo", secs / 2592000)
    };
    Some(format!("{} ago", text))
}
