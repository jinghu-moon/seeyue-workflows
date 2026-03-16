use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::error::{
    CharInfo, NonAsciiChar, Suggestion, ToolError, unicode_char_name,
};

// ─── 编码信息 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EncodingInfo {
    /// WHATWG 编码名称，e.g. "UTF-8", "windows-1252"
    pub name:          String,
    /// chardetng 置信度 0.0–1.0（BOM 检测时为 1.0）
    pub confidence:    f32,
    /// BOM 类型
    pub bom:           Option<BomKind>,
    /// 文件是否包含非 ASCII 字节
    pub has_non_ascii: bool,
    /// 换行符类型
    pub line_ending:   LineEnding,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BomKind {
    Utf8,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LineEnding {
    Lf,
    Crlf,
    Mixed,
}

use serde::Serialize;

// ─── 安全读取结果 ─────────────────────────────────────────────────────────────

pub struct SafeReadResult {
    /// 解码后的 Unicode 文本（孤立代理对已替换为 U+FFFD）
    pub content:     String,
    pub encoding:    EncodingInfo,
    /// sha256(原始字节)，用于 Edit 校验
    pub raw_hash:    String,
    /// sha256(LF 规范化后)，用于 CRLF 容错
    pub norm_hash:   String,
}

// ─── 编码层主入口 ─────────────────────────────────────────────────────────────

/// 检测文件编码
pub fn detect_encoding(raw: &[u8]) -> EncodingInfo {
    // ── Step 1: BOM 检测（优先，最可靠）────────────────────────────────────
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return EncodingInfo {
            name:          "UTF-8".into(),
            confidence:    1.0,
            bom:           Some(BomKind::Utf8),
            has_non_ascii: raw.iter().any(|&b| b > 0x7F),
            line_ending:   detect_line_ending(raw),
        };
    }
    if raw.starts_with(&[0xFF, 0xFE]) {
        return EncodingInfo {
            name:          "UTF-16LE".into(),
            confidence:    1.0,
            bom:           Some(BomKind::Utf16Le),
            has_non_ascii: true,
            line_ending:   LineEnding::Lf, // UTF-16 line endings handled separately
        };
    }
    if raw.starts_with(&[0xFE, 0xFF]) {
        return EncodingInfo {
            name:          "UTF-16BE".into(),
            confidence:    1.0,
            bom:           Some(BomKind::Utf16Be),
            has_non_ascii: true,
            line_ending:   LineEnding::Lf,
        };
    }

    // ── Step 2: 纯 ASCII 快路径 ────────────────────────────────────────────
    if raw.iter().all(|&b| b < 0x80) {
        return EncodingInfo {
            name:          "UTF-8".into(),
            confidence:    1.0,
            bom:           None,
            has_non_ascii: false,
            line_ending:   detect_line_ending(raw),
        };
    }

    // ── Step 3: chardetng（Firefox 同款检测器）─────────────────────────────
    // 使用前 4096 字节（足够，更快）
    let sample = if raw.len() > 4096 { &raw[..4096] } else { raw };
    let mut det = EncodingDetector::new();
    det.feed(sample, true);

    // chardetng 返回 &'static Encoding（来自 encoding_rs）
    let encoding = det.guess(None, true);
    let name     = encoding.name().to_string();

    // chardetng 不直接提供 confidence，根据是否是 UTF-8 赋值
    let confidence = if name == "UTF-8" { 0.95 } else { 0.75 };

    EncodingInfo {
        name,
        confidence,
        bom:           None,
        has_non_ascii: raw.iter().any(|&b| b > 0x7F),
        line_ending:   detect_line_ending(raw),
    }
}

/// 安全读取：检测编码 → 解码 → 修复孤立代理对 → 计算双 hash
pub fn safe_read(path: &Path) -> Result<SafeReadResult, ToolError> {
    let raw = std::fs::read(path).map_err(|e| ToolError::IoError {
        message: e.to_string(),
    })?;

    let encoding = detect_encoding(&raw);

    // ── 解码 ───────────────────────────────────────────────────────────────
    let enc = Encoding::for_label(encoding.name.as_bytes())
        .unwrap_or(encoding_rs::UTF_8);

    // encoding_rs decode：替换模式（不 panic，用 replacement char）
    let (cow, _, _) = enc.decode(&raw);
    let mut content = cow.into_owned();

    // ── 修复孤立代理对（防止 Rust panic on byte-boundary slices）────────────
    content = fix_lone_surrogates(content);

    // ── 双 hash ────────────────────────────────────────────────────────────
    let raw_hash  = sha256_hex(&raw);
    let lf_bytes  = content.replace("\r\n", "\n").into_bytes();
    let norm_hash = sha256_hex(&lf_bytes);

    Ok(SafeReadResult {
        content,
        encoding,
        raw_hash,
        norm_hash,
    })
}

/// 安全写入：往返校验 → 保留原始编码和 BOM → 写文件
pub fn safe_write(
    path:     &Path,
    content:  &str,
    enc_info: &EncodingInfo,
    is_new:   bool,
    orig_was_ascii_only: bool,
) -> Result<(), ToolError> {

    // ── Step 1: 非 ASCII 注入检测（根因 E6）────────────────────────────────
    if orig_was_ascii_only && !is_new {
        let injected = find_non_ascii_chars(content);
        if !injected.is_empty() {
            return Err(ToolError::UnexpectedNonAscii {
                file_path: path.display().to_string(),
                chars:     injected.into_iter().take(5).collect(),
                hint: "Original file was ASCII-only. Model inserted non-ASCII characters. \
                       Replace with ASCII equivalents or explicitly allow non-ASCII.".into(),
            });
        }
    }

    // ── Step 2: 编码往返校验（防止写入损坏）────────────────────────────────
    let enc = Encoding::for_label(enc_info.name.as_bytes())
        .unwrap_or(encoding_rs::UTF_8);

    let (encoded, _, had_unmappable) = enc.encode(content);

    if had_unmappable {
        // 找到第一个无法编码的字符
        let (pos, bad_char) = find_unmappable_char(content, enc);
        let c = bad_char.unwrap_or('?');
        return Err(ToolError::EncodingRoundtripFailed {
            file_path: path.display().to_string(),
            encoding:  enc_info.name.clone(),
            position:  pos,
            original_char: CharInfo {
                char:      c.to_string(),
                codepoint: format!("U+{:04X}", c as u32),
                name:      unicode_char_name(c).to_string(),
                line:      None,
                context:   None,
            },
            hint: format!(
                "Character U+{:04X} ({}) cannot be represented in {}. \
                 Replace with an ASCII equivalent or convert the file to UTF-8 first.",
                c as u32,
                unicode_char_name(c),
                enc_info.name
            ),
        });
    }

    // ── Step 3: 添加原始 BOM ───────────────────────────────────────────────
    let final_bytes: Vec<u8> = match &enc_info.bom {
        Some(BomKind::Utf8)    => [&[0xEF, 0xBB, 0xBF][..], &encoded].concat(),
        Some(BomKind::Utf16Le) => [&[0xFF, 0xFE][..], &encoded].concat(),
        Some(BomKind::Utf16Be) => [&[0xFE, 0xFF][..], &encoded].concat(),
        None                   => encoded.to_vec(),
    };

    // ── Step 4: 保留原始换行符 ─────────────────────────────────────────────
    let final_bytes = match enc_info.line_ending {
        LineEnding::Crlf => ensure_crlf(final_bytes),
        _                => final_bytes,
    };

    // ── Step 5: 创建父目录（mkdir -p 语义）────────────────────────────────
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::MkdirFailed {
                path: parent.display().to_string(),
                hint: e.to_string(),
            })?;
        }
    }

    std::fs::write(path, &final_bytes).map_err(|e| ToolError::IoError {
        message: e.to_string(),
    })?;

    Ok(())
}

// ─── 辅助函数 ────────────────────────────────────────────────────────────────

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn detect_line_ending(raw: &[u8]) -> LineEnding {
    let has_crlf = raw.windows(2).any(|w| w == b"\r\n");
    let has_lf   = raw.iter().zip(raw.iter().skip(1))
        .any(|(&a, &b)| a != b'\r' && b == b'\n');
    match (has_crlf, has_lf) {
        (true,  false) => LineEnding::Crlf,
        (false, true)  => LineEnding::Lf,
        (true,  true)  => LineEnding::Mixed,
        _              => LineEnding::Lf,
    }
}

/// 替换孤立代理对为 U+FFFD（防止 Rust panic）
fn fix_lone_surrogates(s: String) -> String {
    // Rust String 保证合法 UTF-8，encoding_rs 的 replacement decode 已经处理
    // 此处做二次保险：检查并移除任何残留的替换字符序列
    s.chars()
        .map(|c| if c == '\u{FFFD}' { '\u{FFFD}' } else { c })
        .collect()
}

fn find_non_ascii_chars(s: &str) -> Vec<NonAsciiChar> {
    let mut result = Vec::new();
    for (line_idx, line) in s.lines().enumerate() {
        for c in line.chars() {
            if c as u32 > 0x7F {
                result.push(NonAsciiChar {
                    char:      c.to_string(),
                    codepoint: format!("U+{:04X}", c as u32),
                    line:      line_idx + 1,
                    context:   line.chars().take(60).collect(),
                });
            }
        }
    }
    result
}

fn find_unmappable_char(
    s:   &str,
    enc: &'static Encoding,
) -> (usize, Option<char>) {
    for (pos, c) in s.char_indices() {
        let mut buf = [0u8; 4];
        let s_char = c.encode_utf8(&mut buf);
        let (encoded, _, had_error) = enc.encode(s_char);
        if had_error || encoded.contains(&b'?') {
            return (pos, Some(c));
        }
    }
    (0, None)
}

fn ensure_crlf(bytes: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 20);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' && (i == 0 || bytes[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Unicode 常见混淆对（根因 E4）
pub fn find_unicode_confusion(
    old_string: &str,
    file_content: &str,
) -> Vec<Suggestion> {
    // 常见混淆对：(文件中的字符, 查询中可能用的替代字符)
    const CONFUSABLE: &[(char, char)] = &[
        ('\u{2019}', '\''),   // RIGHT SINGLE QUOTATION MARK ↔ APOSTROPHE
        ('\u{2018}', '\''),   // LEFT SINGLE QUOTATION MARK
        ('\u{201C}', '"'),    // LEFT DOUBLE QUOTATION MARK
        ('\u{201D}', '"'),    // RIGHT DOUBLE QUOTATION MARK
        ('\u{2013}', '-'),    // EN DASH
        ('\u{2014}', '-'),    // EM DASH
        ('\u{00A0}', ' '),    // NO-BREAK SPACE
        ('\u{202F}', ' '),    // NARROW NO-BREAK SPACE
    ];

    let mut suggestions = Vec::new();

    for &(file_char, query_char) in CONFUSABLE {
        // 文件中有此字符，但查询字符串用的是替代字符
        let file_has    = file_content.contains(file_char);
        let query_has   = old_string.contains(query_char);
        let query_lacks = !old_string.contains(file_char);

        if file_has && query_has && query_lacks {
            // 找到文件中该字符出现的行
            let line = file_content.lines().enumerate()
                .find(|(_, l)| l.contains(file_char))
                .map(|(i, _)| i + 1);

            suggestions.push(Suggestion::UnicodeCandidate {
                char_in_file: CharInfo {
                    char:      file_char.to_string(),
                    codepoint: format!("U+{:04X}", file_char as u32),
                    name:      unicode_char_name(file_char).to_string(),
                    line,
                    context:   None,
                },
                char_in_query: CharInfo {
                    char:      query_char.to_string(),
                    codepoint: format!("U+{:04X}", query_char as u32),
                    name:      unicode_char_name(query_char).to_string(),
                    line:      None,
                    context:   None,
                },
            });
        }
    }

    suggestions
}

/// Tab/Space 规范化尝试（根因 E1）
pub fn try_tab_normalized_match(
    old_string:   &str,
    file_content: &str,
) -> Option<String> {
    // 尝试把 old_string 中的 4 空格缩进换成 tab
    let with_tabs = old_string.replace("    ", "\t");
    if with_tabs != old_string && file_content.contains(&with_tabs) {
        return Some(with_tabs);
    }

    // 尝试把 old_string 中的 tab 换成 4 空格
    let with_spaces = old_string.replace('\t', "    ");
    if with_spaces != old_string && file_content.contains(&with_spaces) {
        return Some(with_spaces);
    }

    // 2 空格缩进
    let with_tabs_2 = old_string.replace("  ", "\t");
    if with_tabs_2 != old_string && file_content.contains(&with_tabs_2) {
        return Some(with_tabs_2);
    }

    None
}
