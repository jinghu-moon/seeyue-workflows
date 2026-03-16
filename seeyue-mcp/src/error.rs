use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── 结构化错误（Agent 可程序性解析）────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolError {
    // ── Read ──────────────────────────────────────────────────────────────
    FileNotFound {
        file_path: String,
        hint:      String,
    },
    PathEscape {
        file_path: String,
        hint:      String,
    },
    BinaryFile {
        file_path: String,
        hint:      String,
    },
    InvalidLineRange {
        start_line:  usize,
        end_line:    usize,
        total_lines: usize,
        hint:        String,
    },

    // ── Edit ──────────────────────────────────────────────────────────────
    StringNotFound {
        file_path:         String,
        old_string_preview: String,
        suggestions:       Vec<Suggestion>,
        hint:              String,
    },
    MultipleMatches {
        count:     usize,
        locations: Vec<MatchLocation>,
        hint:      String,
    },
    NoChanges {
        hint:         String,
        unicode_note: Option<String>,
    },
    FileModified {
        file_path: String,
        read_at:   String,
        hint:      String,
        tip:       String,
    },
    FileNotRead {
        file_path: String,
        hint:      String,
    },
    MissingParameter {
        missing: String,
        hint:    String,
    },

    // ── MultiEdit ─────────────────────────────────────────────────────────
    EditFailed {
        edit_index:   usize,
        edit_preview: EditPreview,
        cause:        Box<ToolError>,
        file_state:   String,
        hint:         String,
    },

    // ── 编码 ──────────────────────────────────────────────────────────────
    EncodingAmbiguous {
        file_path:  String,
        candidates: Vec<EncodingCandidate>,
        hint:       String,
        suggestion: String,
    },
    EncodingRoundtripFailed {
        file_path:     String,
        encoding:      String,
        position:      usize,
        original_char: CharInfo,
        hint:          String,
    },
    UnexpectedNonAscii {
        file_path: String,
        chars:     Vec<NonAsciiChar>,
        hint:      String,
    },
    InvalidSurrogate {
        file_path: String,
        position:  usize,
        hint:      String,
    },
    FilenameUnicodeUnsupported {
        file_path:         String,
        problematic_char:  CharInfo,
        hint:              String,
    },

    // ── Write ─────────────────────────────────────────────────────────────
    MkdirFailed {
        path: String,
        hint: String,
    },

    // ── 系统级 ────────────────────────────────────────────────────────────
    IoError {
        message: String,
    },

    // ── P1: Policy Engine ─────────────────────────────────────────────────
    PolicyError {
        phase:   String,
        message: String,
        hint:    String,
    },
    ResourceNotFound {
        uri:  String,
        hint: String,
    },

    // ── P2: Tree-sitter ──────────────────────────────────────────────────
    UnsupportedLanguage {
        language: String,
        hint:     String,
    },
    SyntaxError {
        language: String,
        errors:   Vec<SyntaxIssue>,
        hint:     String,
    },

    // ── P2: Search ───────────────────────────────────────────────────────
    InvalidRegex {
        pattern: String,
        message: String,
        hint:    String,
    },

    // ── P2: Git ──────────────────────────────────────────────────────────
    GitNotAvailable {
        hint: String,
    },
    GitNotRepo {
        hint: String,
    },
    GitError {
        message: String,
        hint:    String,
    },

    // ── P2: LSP ──────────────────────────────────────────────────────────
    LspNotAvailable {
        language: String,
        hint:     String,
    },
    LspTimeout {
        language: String,
        hint:     String,
    },
    LspError {
        message: String,
        hint:    String,
    },

    // ── P2: Skills Prompts ───────────────────────────────────────────────
    SkillNotFound {
        name: String,
        hint: String,
    },
}

impl ToolError {
    /// 序列化为 Agent 可解析的 JSON 字符串
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| {
            r#"{"error":"SERIALIZATION_FAILED"}"#.to_string()
        })
    }
}

// ─── 子数据结构 ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Suggestion {
    TabSpaceMismatch { hint: String },
    UnicodeCandidate {
        char_in_file:  CharInfo,
        char_in_query: CharInfo,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchLocation {
    pub line:    usize,
    pub context: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EditPreview {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyntaxIssue {
    pub line:    usize,
    pub column:  usize,
    pub kind:    String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncodingCandidate {
    pub encoding:   String,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CharInfo {
    pub char:      String,
    pub codepoint: String,  // "U+2019"
    pub name:      String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line:      Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context:   Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NonAsciiChar {
    pub char:      String,
    pub codepoint: String,
    pub line:      usize,
    pub context:   String,
}

// ─── thiserror（内部错误，转换为 ToolError）──────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum InternalError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Tool error: {0}")]
    Tool(String),
}

impl From<ToolError> for InternalError {
    fn from(e: ToolError) -> Self {
        InternalError::Tool(e.to_json())
    }
}

/// 字符名称（简化版，覆盖常见混淆字符）
pub fn unicode_char_name(c: char) -> &'static str {
    match c {
        '\u{2019}' => "RIGHT SINGLE QUOTATION MARK",
        '\u{2018}' => "LEFT SINGLE QUOTATION MARK",
        '\u{201C}' => "LEFT DOUBLE QUOTATION MARK",
        '\u{201D}' => "RIGHT DOUBLE QUOTATION MARK",
        '\u{2013}' => "EN DASH",
        '\u{2014}' => "EM DASH",
        '\u{2192}' => "RIGHTWARDS ARROW",
        '\u{2190}' => "LEFTWARDS ARROW",
        '\u{2713}' => "CHECK MARK",
        '\u{202F}' => "NARROW NO-BREAK SPACE",
        '\u{00A0}' => "NO-BREAK SPACE",
        '\u{FEFF}' => "ZERO WIDTH NO-BREAK SPACE (BOM)",
        _          => "UNKNOWN",
    }
}
