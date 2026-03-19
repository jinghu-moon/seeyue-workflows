// tests/ts_symbols.rs
//
// TDD tests for treesitter::symbols::TsSymbol and extract_ts_symbols().
// Red phase: compile fails until TsSymbol and extract_ts_symbols are implemented.
// Run: cargo test --test ts_symbols

use seeyue_mcp::treesitter::symbols::{TsSymbol, extract_ts_symbols};

// A-N2 test 1: top-level functions in Rust source
#[test]
fn test_extract_top_level_fns_rust() {
    let src = r#"
fn foo() -> i32 { 42 }
fn bar(x: i32) -> i32 { x + 1 }
"#;
    let syms = extract_ts_symbols(src, "rust");
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"foo"), "expected foo, got: {:?}", names);
    assert!(names.contains(&"bar"), "expected bar, got: {:?}", names);
}

// A-N2 test 2: impl block methods as children of struct
#[test]
fn test_extract_impl_methods_as_children() {
    let src = r#"
struct MyStruct;
impl MyStruct {
    pub fn new() -> Self { MyStruct }
    fn helper(&self) {}
}
"#;
    let syms = extract_ts_symbols(src, "rust");
    // Find MyStruct (or the impl block representing it)
    let parent = syms.iter().find(|s| s.name == "MyStruct");
    assert!(parent.is_some(), "expected MyStruct symbol, got: {:?}", syms.iter().map(|s| &s.name).collect::<Vec<_>>());
    let parent = parent.unwrap();
    let child_names: Vec<&str> = parent.children.iter().map(|c| c.name.as_str()).collect();
    assert!(child_names.contains(&"new"), "expected new in children, got: {:?}", child_names);
    assert!(child_names.contains(&"helper"), "expected helper in children, got: {:?}", child_names);
}

// A-N2 test 3: to_name_path() single level
#[test]
fn test_name_path_single_level() {
    let sym = TsSymbol {
        name: "my_fn".into(),
        kind: "function".into(),
        start_line: 1,
        end_line: 3,
        children: vec![],
    };
    assert_eq!(sym.to_name_path(None), "my_fn");
}

// A-N2 test 4: to_name_path() with parent
#[test]
fn test_name_path_with_parent() {
    let sym = TsSymbol {
        name: "do_thing".into(),
        kind: "method".into(),
        start_line: 5,
        end_line: 8,
        children: vec![],
    };
    assert_eq!(sym.to_name_path(Some("MyStruct")), "MyStruct/do_thing");
}

// A-N2 test 5: empty source returns empty
#[test]
fn test_empty_source() {
    let syms = extract_ts_symbols("", "rust");
    assert!(syms.is_empty());
}

// A-N2 test 6: unsupported language returns empty (graceful fallback)
#[test]
fn test_unsupported_language_returns_empty() {
    let syms = extract_ts_symbols("some code", "cobol");
    // Should not panic; empty or fallback result is acceptable
    let _ = syms; // just verify no panic
}

// A-N2 test 7: CRLF line endings — line numbers stay correct
#[test]
fn test_crlf_line_numbers() {
    let src = "fn first() {\r\n    42\r\n}\r\nfn second() {\r\n    0\r\n}\r\n";
    let syms = extract_ts_symbols(src, "rust");
    let first = syms.iter().find(|s| s.name == "first");
    let second = syms.iter().find(|s| s.name == "second");
    assert!(first.is_some(), "expected first fn");
    assert!(second.is_some(), "expected second fn");
    // second starts after first ends
    if let (Some(f), Some(s)) = (first, second) {
        assert!(s.start_line > f.start_line, "second should start after first");
    }
}
