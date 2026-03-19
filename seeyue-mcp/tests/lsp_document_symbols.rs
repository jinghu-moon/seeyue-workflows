// tests/lsp_document_symbols.rs
//
// TDD tests for LspSession::request_document_symbols().
// Red phase: these tests will fail to compile until the method is implemented.
// Run: cargo test --test lsp_document_symbols

use seeyue_mcp::lsp::{LspSymbol, LspSymbolKind, parse_document_symbols};

// A-N1 test 1: DocumentSymbol nested format
#[test]
fn test_parse_document_symbol_nested() {
    let response = serde_json::json!([
        {
            "name": "MyStruct",
            "kind": 23,
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 10, "character": 1 } },
            "selectionRange": { "start": { "line": 0, "character": 7 }, "end": { "line": 0, "character": 15 } },
            "children": [
                {
                    "name": "new",
                    "kind": 6,
                    "range": { "start": { "line": 2, "character": 4 }, "end": { "line": 5, "character": 5 } },
                    "selectionRange": { "start": { "line": 2, "character": 7 }, "end": { "line": 2, "character": 10 } },
                    "children": []
                }
            ]
        }
    ]);

    let symbols = parse_document_symbols(&response);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "MyStruct");
    assert_eq!(symbols[0].kind, LspSymbolKind::Struct);
    assert_eq!(symbols[0].start_line, 1); // LSP 0-indexed -> internal 1-indexed
    assert_eq!(symbols[0].end_line, 11);
    assert_eq!(symbols[0].children.len(), 1);
    assert_eq!(symbols[0].children[0].name, "new");
    assert_eq!(symbols[0].children[0].kind, LspSymbolKind::Method);
    assert_eq!(symbols[0].children[0].start_line, 3);
}

// A-N1 test 2: SymbolInformation flat format
#[test]
fn test_parse_symbol_information_flat() {
    let response = serde_json::json!([
        {
            "name": "top_level_fn",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/lib.rs",
                "range": { "start": { "line": 4, "character": 0 }, "end": { "line": 8, "character": 1 } }
            }
        },
        {
            "name": "another_fn",
            "kind": 12,
            "location": {
                "uri": "file:///workspace/src/lib.rs",
                "range": { "start": { "line": 10, "character": 0 }, "end": { "line": 14, "character": 1 } }
            }
        }
    ]);

    let symbols = parse_document_symbols(&response);
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].name, "top_level_fn");
    assert_eq!(symbols[0].kind, LspSymbolKind::Function);
    assert_eq!(symbols[0].start_line, 5);
    assert!(symbols[0].children.is_empty());
    assert_eq!(symbols[1].name, "another_fn");
    assert_eq!(symbols[1].start_line, 11);
}

// A-N1 test 3: empty array
#[test]
fn test_parse_document_symbols_empty() {
    let response = serde_json::json!([]);
    let symbols = parse_document_symbols(&response);
    assert!(symbols.is_empty());
}

// A-N1 test 4: null result (LSP returned null)
#[test]
fn test_parse_document_symbols_null() {
    let response = serde_json::json!(null);
    let symbols = parse_document_symbols(&response);
    assert!(symbols.is_empty());
}

// A-N1 test 5: unknown symbol kind falls back to Other
#[test]
fn test_parse_unknown_symbol_kind() {
    let response = serde_json::json!([
        {
            "name": "WeirdThing",
            "kind": 999,
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
            "selectionRange": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 9 } },
            "children": []
        }
    ]);
    let symbols = parse_document_symbols(&response);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].kind, LspSymbolKind::Other);
}

// A-N1 test 6: name_path() single-level
#[test]
fn test_lsp_symbol_name_path_single() {
    let sym = LspSymbol {
        name: "my_func".into(),
        kind: LspSymbolKind::Function,
        start_line: 1,
        end_line: 5,
        children: vec![],
    };
    assert_eq!(sym.name_path(None), "my_func");
}

// A-N1 test 7: name_path() with parent prefix
#[test]
fn test_lsp_symbol_name_path_with_parent() {
    let sym = LspSymbol {
        name: "new".into(),
        kind: LspSymbolKind::Method,
        start_line: 2,
        end_line: 6,
        children: vec![],
    };
    assert_eq!(sym.name_path(Some("MyStruct")), "MyStruct/new");
}
