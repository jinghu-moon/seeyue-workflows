// src/treesitter/symbols.rs
//
// Symbol extraction: dispatch by language, language-specific parsers.

use serde::Serialize;

use crate::treesitter::languages::{grammar_for, TsLanguage};
use super::helpers::{
    collect_by_kind, find_child_text, find_name, find_parent_name,
    sig_up_to_block, unwrap_decorated,
};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct Symbol {
    pub kind: String,
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub fn extract_symbols(language: &str, source: &str, depth: u8) -> Vec<Symbol> {
    match language {
        "rust"       => parse_rust(source, depth),
        "python"     => parse_python(source, depth),
        "typescript" | "tsx" => parse_typescript(source, language, depth),
        "go"         => parse_go(source, depth),
        _             => parse_regex_fallback(source, language),
    }
}

pub fn estimate_tokens(symbols: &[Symbol]) -> usize {
    let total_chars: usize = symbols
        .iter()
        .filter_map(|s| s.signature.as_ref())
        .map(|s| s.len())
        .sum();
    total_chars / 4
}

// ─── Rust ────────────────────────────────────────────────────────────────────

fn parse_rust(source: &str, depth: u8) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&grammar_for(TsLanguage::Rust)).is_err() {
        return parse_regex_fallback(source, "rust");
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return parse_regex_fallback(source, "rust"),
    };
    let src = source.as_bytes();
    let nodes = collect_by_kind(tree.root_node(), &[
        "function_item","impl_item","struct_item","enum_item",
        "trait_item","type_item","const_item","mod_item",
    ]);

    nodes.iter().filter_map(|n| {
        let node = tree.root_node().descendant_for_byte_range(n.start_byte, n.end_byte)?;
        let name = find_name(node, src, &["identifier", "type_identifier"])?;

        let parent = if node.kind() == "function_item" {
            find_parent_name(node, src, &["impl_item", "trait_item"], &["type_identifier"])
        } else {
            None
        };

        if depth == 0 && parent.is_some() {
            return None;
        }

        let sig = if node.kind() == "function_item" {
            sig_up_to_block(node, "block", src)
        } else { None };

        let vis = find_child_text(node, "visibility_modifier", src);

        let kind = match node.kind() {
            "function_item" => "fn",
            "impl_item"     => "impl",
            "struct_item"   => "struct",
            "enum_item"     => "enum",
            "trait_item"    => "trait",
            "type_item"     => "type",
            "const_item"    => "const",
            "mod_item"      => "mod",
            other           => other,
        };

        Some(Symbol {
            kind: kind.to_string(),
            name,
            line: n.start_line,
            end_line: n.end_line,
            signature: sig,
            visibility: vis,
            parent,
            source: None,
        })
    }).collect()
}

// ─── Python ──────────────────────────────────────────────────────────────────

fn parse_python(source: &str, depth: u8) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&grammar_for(TsLanguage::Python)).is_err() {
        return parse_regex_fallback(source, "python");
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return parse_regex_fallback(source, "python"),
    };
    let src = source.as_bytes();
    let nodes = collect_by_kind(tree.root_node(), &[
        "class_definition","function_definition","decorated_definition",
    ]);

    nodes.iter().filter_map(|n| {
        let node = tree.root_node().descendant_for_byte_range(n.start_byte, n.end_byte)?;
        let inner = if node.kind() == "decorated_definition" {
            unwrap_decorated(node)
        } else {
            Some(node)
        }?;

        let name = find_child_text(inner, "identifier", src)?;
        let parent = if inner.kind() == "function_definition" {
            find_parent_name(inner, src, &["class_definition"], &["identifier"])
        } else { None };

        if depth == 0 && parent.is_some() {
            return None;
        }

        let sig = if inner.kind() == "function_definition" {
            sig_up_to_block(inner, "block", src)
                .map(|s| s.trim_end_matches(':').trim().to_string())
        } else { None };

        let vis = if name.starts_with("__") && !name.ends_with("__") {
            Some("private".into())
        } else if name.starts_with('_') {
            Some("protected".into())
        } else {
            Some("public".into())
        };

        let kind = match inner.kind() {
            "class_definition" => "class",
            _                  => "fn",
        };

        Some(Symbol {
            kind: kind.into(),
            name,
            line: n.start_line,
            end_line: n.end_line,
            signature: sig,
            visibility: vis,
            parent,
            source: None,
        })
    }).collect()
}

// ─── TypeScript / TSX ────────────────────────────────────────────────────────

fn parse_typescript(source: &str, lang: &str, depth: u8) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    let grammar = if lang == "tsx" {
        grammar_for(TsLanguage::Tsx)
    } else {
        grammar_for(TsLanguage::TypeScript)
    };
    if parser.set_language(&grammar).is_err() {
        return parse_regex_fallback(source, "typescript");
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return parse_regex_fallback(source, "typescript"),
    };
    let src = source.as_bytes();
    let nodes = collect_by_kind(tree.root_node(), &[
        "function_declaration","method_definition","class_declaration",
        "interface_declaration","type_alias_declaration","abstract_class_declaration",
    ]);

    nodes.iter().filter_map(|n| {
        let node = tree.root_node().descendant_for_byte_range(n.start_byte, n.end_byte)?;
        let name = find_name(node, src, &["identifier", "type_identifier", "property_identifier"])?;
        let parent = if node.kind() == "method_definition" {
            find_parent_name(node, src, &["class_declaration", "abstract_class_declaration"], &["identifier"])
        } else { None };

        if depth == 0 && parent.is_some() {
            return None;
        }

        let sig = sig_up_to_block(node, "statement_block", src);

        let kind = match node.kind() {
            "function_declaration" => "fn",
            "method_definition"    => "method",
            "class_declaration" | "abstract_class_declaration" => "class",
            "interface_declaration" => "interface",
            "type_alias_declaration" => "type",
            other => other,
        };

        Some(Symbol {
            kind: kind.into(),
            name,
            line: n.start_line,
            end_line: n.end_line,
            signature: sig,
            visibility: None,
            parent,
            source: None,
        })
    }).collect()
}

// ─── Go ──────────────────────────────────────────────────────────────────────

fn parse_go(source: &str, depth: u8) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&grammar_for(TsLanguage::Go)).is_err() {
        return parse_regex_fallback(source, "go");
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return parse_regex_fallback(source, "go"),
    };
    let src = source.as_bytes();
    let nodes = collect_by_kind(tree.root_node(), &[
        "function_declaration","method_declaration","type_declaration",
        "const_declaration","var_declaration",
    ]);

    nodes.iter().filter_map(|n| {
        let node = tree.root_node().descendant_for_byte_range(n.start_byte, n.end_byte)?;
        let name = find_name(node, src, &["identifier", "field_identifier"])?;
        let is_method = node.kind() == "method_declaration";

        if depth == 0 && is_method {
            return None;
        }

        let sig = sig_up_to_block(node, "block", src);
        let vis = if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            Some("exported".into())
        } else {
            Some("unexported".into())
        };

        let kind = match node.kind() {
            "function_declaration" => "fn",
            "method_declaration"   => "method",
            "type_declaration"     => "type",
            "const_declaration"    => "const",
            "var_declaration"      => "var",
            other => other,
        };

        Some(Symbol {
            kind: kind.into(),
            name,
            line: n.start_line,
            end_line: n.end_line,
            signature: sig,
            visibility: vis,
            parent: None,
            source: None,
        })
    }).collect()
}

// ─── Regex fallback ──────────────────────────────────────────────────────────

pub fn parse_regex_fallback(source: &str, lang: &str) -> Vec<Symbol> {
    use regex::Regex;
    let lines: Vec<&str> = source.lines().collect();
    let mut symbols = Vec::new();
    let patterns: &[(&str, &str)] = match lang {
        "javascript" | "jsx" => &[
            (r"^(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][\w$]*)", "fn"),
            (r"^(?:export\s+)?class\s+([A-Za-z_$][\w$]*)", "class"),
            (r"^(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s+)?\(", "fn"),
        ],
        "c" | "cpp" => &[
            (r"^(?:static\s+|inline\s+|extern\s+)*[\w:*&<>]+\s+([A-Za-z_][\w:~]*)\s*\(", "fn"),
            (r"^(?:class|struct|enum)\s+([A-Za-z_]\w*)", "class"),
        ],
        _ => &[
            (r"^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:fn|function|def|func)\s+([A-Za-z_][\w]*)", "fn"),
            (r"^(?:class|struct|interface|trait|enum|impl)\s+([A-Za-z_][\w]*)", "type"),
        ],
    };

    for (i, line) in lines.iter().enumerate() {
        for (pat, kind) in patterns {
            if let Ok(re) = Regex::new(pat) {
                if let Some(cap) = re.captures(line) {
                    let name = cap.get(cap.len() - 1)
                        .map(|m| m.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !name.is_empty() {
                        symbols.push(Symbol {
                            kind: kind.to_string(),
                            name,
                            line: i + 1,
                            end_line: i + 1,
                            signature: Some(line.trim().to_string()),
                            visibility: None,
                            parent: None,
                            source: Some("regex_fallback".into()),
                        });
                    }
                }
            }
        }
    }

    symbols
}
