// src/treesitter/helpers.rs
//
// Shared tree-sitter helper types and functions used by all language parsers.

use tree_sitter::Node;

// ─── NodeInfo ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct NodeInfo {
    pub start_byte: usize,
    pub end_byte:   usize,
    pub start_line: usize,
    pub end_line:   usize,
}

// ─── Tree traversal ──────────────────────────────────────────────────────────

pub(super) fn collect_by_kind(root: Node, kinds: &[&str]) -> Vec<NodeInfo> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if kinds.contains(&node.kind()) {
            out.push(NodeInfo {
                start_byte: node.start_byte(),
                end_byte:   node.end_byte(),
                start_line: node.start_position().row + 1,
                end_line:   node.end_position().row + 1,
            });
        }
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }
    out.sort_by_key(|n| n.start_byte);
    out
}

// ─── Node text extraction ────────────────────────────────────────────────────

pub(super) fn find_child_text(node: Node, kind: &str, src: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return child.utf8_text(src).ok().map(|s| s.to_string());
            }
        }
    }
    None
}

pub(super) fn find_name(node: Node, src: &[u8], kinds: &[&str]) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if kinds.contains(&child.kind()) {
                return child.utf8_text(src).ok().map(|s| s.to_string());
            }
        }
    }
    None
}

pub(super) fn find_parent_name(
    node: Node,
    src: &[u8],
    parent_kinds: &[&str],
    name_kinds: &[&str],
) -> Option<String> {
    let mut cur = node.parent();
    while let Some(p) = cur {
        if parent_kinds.contains(&p.kind()) {
            return find_name(p, src, name_kinds);
        }
        cur = p.parent();
    }
    None
}

pub(super) fn unwrap_decorated(node: Node) -> Option<Node> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "function_definition" || child.kind() == "class_definition" {
                return Some(child);
            }
        }
    }
    None
}

// ─── Signature extraction ────────────────────────────────────────────────────

pub(super) fn sig_up_to_block(node: Node, block_kind: &str, src: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == block_kind {
                let raw = &src[node.start_byte()..child.start_byte()];
                return Some(compact_ws(String::from_utf8_lossy(raw).as_ref()));
            }
        }
    }
    None
}

pub(super) fn compact_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
