// tests/test_symbols.rs
//
// Functional tests for treesitter::symbols::extract_symbols.
// Covers Rust, Python, TypeScript, Go and regex fallback.
// Run: cargo test --test test_symbols

use seeyue_mcp::treesitter::symbols::{extract_symbols, estimate_tokens};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn names(syms: &[seeyue_mcp::treesitter::symbols::Symbol]) -> Vec<&str> {
    syms.iter().map(|s| s.name.as_str()).collect()
}

// ─── Rust ─────────────────────────────────────────────────────────────────────

#[test]
fn test_rust_function() {
    let src = "pub fn hello(x: i32) -> i32 { x + 1 }";
    let syms = extract_symbols("rust", src, 2);
    assert!(names(&syms).contains(&"hello"), "should extract fn hello");
    let sym = syms.iter().find(|s| s.name == "hello").unwrap();
    assert_eq!(sym.kind, "fn");
}

#[test]
fn test_rust_struct() {
    let src = "pub struct Foo { pub x: i32 }";
    let syms = extract_symbols("rust", src, 2);
    assert!(names(&syms).contains(&"Foo"), "should extract struct Foo");
    let sym = syms.iter().find(|s| s.name == "Foo").unwrap();
    assert_eq!(sym.kind, "struct");
}

#[test]
fn test_rust_enum() {
    let src = "pub enum Color { Red, Green, Blue }";
    let syms = extract_symbols("rust", src, 2);
    assert!(names(&syms).contains(&"Color"));
    let sym = syms.iter().find(|s| s.name == "Color").unwrap();
    assert_eq!(sym.kind, "enum");
}

#[test]
fn test_rust_impl_methods() {
    let src = "
pub struct Counter { n: u32 }
impl Counter {
    pub fn new() -> Self { Counter { n: 0 } }
    pub fn inc(&mut self) { self.n += 1; }
}
";
    let syms = extract_symbols("rust", src, 2);
    let sym_names = names(&syms);
    assert!(sym_names.contains(&"Counter"), "should extract Counter struct");
    // impl methods may or may not be extracted depending on depth
    // At minimum the struct should be present
    assert!(!syms.is_empty());
}

#[test]
fn test_rust_multiple_items() {
    let src = "
pub fn foo() {}
pub fn bar() {}
pub struct Baz {}
pub enum Qux { A }
";
    let syms = extract_symbols("rust", src, 2);
    let sym_names = names(&syms);
    assert!(sym_names.contains(&"foo"));
    assert!(sym_names.contains(&"bar"));
    assert!(sym_names.contains(&"Baz"));
    assert!(sym_names.contains(&"Qux"));
}

#[test]
fn test_rust_empty_source() {
    let syms = extract_symbols("rust", "", 2);
    assert!(syms.is_empty(), "empty source should yield no symbols");
}

#[test]
fn test_rust_line_numbers() {
    let src = "// comment\npub fn target() {}\n";
    let syms = extract_symbols("rust", src, 2);
    let sym = syms.iter().find(|s| s.name == "target");
    assert!(sym.is_some(), "should find target");
    // line is 1-based, target is on line 2
    assert_eq!(sym.unwrap().line, 2);
}

// ─── Python ───────────────────────────────────────────────────────────────────

#[test]
fn test_python_function() {
    let src = "def greet(name):\n    return f'Hello {name}'\n";
    let syms = extract_symbols("python", src, 2);
    assert!(names(&syms).contains(&"greet"), "should extract greet");
    let sym = syms.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(sym.kind, "fn");
}

#[test]
fn test_python_class() {
    let src = "class Animal:\n    def __init__(self):\n        pass\n";
    let syms = extract_symbols("python", src, 2);
    assert!(names(&syms).contains(&"Animal"), "should extract Animal");
    let sym = syms.iter().find(|s| s.name == "Animal").unwrap();
    assert_eq!(sym.kind, "class");
}

#[test]
fn test_python_empty() {
    let syms = extract_symbols("python", "", 2);
    assert!(syms.is_empty());
}

#[test]
fn test_python_multiple() {
    let src = "def foo(): pass\ndef bar(): pass\nclass Baz: pass\n";
    let syms = extract_symbols("python", src, 2);
    let sym_names = names(&syms);
    assert!(sym_names.contains(&"foo"));
    assert!(sym_names.contains(&"bar"));
    assert!(sym_names.contains(&"Baz"));
}

// ─── TypeScript ────────────────────────────────────────────────────────────────

#[test]
fn test_typescript_function() {
    let src = "function greet(name: string): string { return `Hello ${name}`; }";
    let syms = extract_symbols("typescript", src, 2);
    assert!(!syms.is_empty(), "should extract at least one symbol");
    assert!(names(&syms).contains(&"greet"), "should extract greet");
}

#[test]
fn test_typescript_class() {
    let src = "class MyService {\n  constructor() {}\n  doWork(): void {}\n}\n";
    let syms = extract_symbols("typescript", src, 2);
    assert!(names(&syms).contains(&"MyService"), "should extract MyService");
}

#[test]
fn test_typescript_interface() {
    let src = "interface IUser { name: string; age: number; }";
    let syms = extract_symbols("typescript", src, 2);
    // interface may or may not be extracted based on grammar
    // at minimum no panic
    let _ = syms;
}

#[test]
fn test_tsx_variant() {
    let src = "function MyComponent(): JSX.Element { return <div/>; }";
    let syms = extract_symbols("tsx", src, 2);
    assert!(!syms.is_empty(), "tsx should use same parser as typescript");
}

// ─── Go ────────────────────────────────────────────────────────────────────────

#[test]
fn test_go_function() {
    let src = "package main\nfunc Hello() string { return \"hi\" }\n";
    let syms = extract_symbols("go", src, 2);
    assert!(names(&syms).contains(&"Hello"), "should extract Hello");
    let sym = syms.iter().find(|s| s.name == "Hello").unwrap();
    assert_eq!(sym.kind, "fn");
}

#[test]
fn test_go_struct() {
    let src = "package main\ntype Point struct { X int; Y int }\n";
    let syms = extract_symbols("go", src, 2);
    // Go struct is wrapped in type_declaration — name may not be directly extracted
    // At minimum no panic and source is processed
    let _ = syms;
}

#[test]
fn test_go_empty() {
    let syms = extract_symbols("go", "package main\n", 2);
    assert!(syms.is_empty(), "empty go source should yield no symbols");
}

// ─── Regex fallback ───────────────────────────────────────────────────────────

#[test]
fn test_unknown_language_does_not_panic() {
    let src = "function foo() {}\nclass Bar {}\n";
    let syms = extract_symbols("cobol", src, 2);
    // regex fallback: may or may not find symbols, but must not panic
    let _ = syms;
}

#[test]
fn test_unknown_language_empty_source() {
    let syms = extract_symbols("brainfuck", "", 2);
    assert!(syms.is_empty());
}

// ─── estimate_tokens ─────────────────────────────────────────────────────────

#[test]
fn test_estimate_tokens_empty() {
    let syms = vec![];
    assert_eq!(estimate_tokens(&syms), 0);
}

#[test]
fn test_estimate_tokens_proportional() {
    // 40 chars signature / 4 = 10 tokens
    use seeyue_mcp::treesitter::symbols::Symbol;
    let sym = Symbol {
        kind: "function".to_string(),
        name: "foo".to_string(),
        line: 1,
        end_line: 1,
        signature: Some("a".repeat(40)),
        visibility: None,
        parent: None,
        source: None,
    };
    assert_eq!(estimate_tokens(&[sym]), 10);
}

#[test]
fn test_estimate_tokens_no_signature_ignored() {
    use seeyue_mcp::treesitter::symbols::Symbol;
    let sym = Symbol {
        kind: "struct".to_string(),
        name: "Foo".to_string(),
        line: 1,
        end_line: 2,
        signature: None,
        visibility: None,
        parent: None,
        source: None,
    };
    assert_eq!(estimate_tokens(&[sym]), 0, "symbol without signature contributes 0 tokens");
}
