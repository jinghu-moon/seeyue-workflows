use std::path::Path;
use tree_sitter::Language;

#[derive(Debug, Clone, Copy)]
pub enum TsLanguage {
    Rust,
    Python,
    TypeScript,
    Tsx,
    Go,
    Vue,
}

pub fn detect_language(path: &Path) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs"             => "rust",
        "py" | "pyw"    => "python",
        "ts"             => "typescript",
        "tsx"            => "tsx",
        "go"             => "go",
        "vue"            => "vue",
        "js" | "mjs"    => "javascript",
        "jsx"            => "jsx",
        "c" | "h"       => "c",
        "cpp" | "cxx" | "cc" | "hpp" => "cpp",
        "java"           => "java",
        "rb"             => "ruby",
        "swift"          => "swift",
        "kt" | "kts"    => "kotlin",
        "cs"             => "csharp",
        "sh" | "bash"   => "shell",
        _                => "unknown",
    }.to_string()
}

pub fn ts_language(lang: &str) -> Option<TsLanguage> {
    match lang {
        "rust"       => Some(TsLanguage::Rust),
        "python"     => Some(TsLanguage::Python),
        "typescript" => Some(TsLanguage::TypeScript),
        "tsx"        => Some(TsLanguage::Tsx),
        "go"         => Some(TsLanguage::Go),
        "vue"        => Some(TsLanguage::Vue),
        _            => None,
    }
}

pub fn grammar_for(lang: TsLanguage) -> Language {
    match lang {
        TsLanguage::Rust       => tree_sitter_rust::LANGUAGE.into(),
        TsLanguage::Python     => tree_sitter_python::LANGUAGE.into(),
        TsLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        TsLanguage::Tsx        => tree_sitter_typescript::LANGUAGE_TSX.into(),
        TsLanguage::Go         => tree_sitter_go::LANGUAGE.into(),
        TsLanguage::Vue        => tree_sitter_vue_updated::language(),
    }
}
