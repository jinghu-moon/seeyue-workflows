// src/encoding/rw.rs
//
// Safe file read/write: encoding-aware, BOM-preserving, CRLF-preserving.

use encoding_rs::Encoding;
use std::path::Path;

use crate::error::{CharInfo, ToolError, unicode_char_name};
use super::detect::{EncodingInfo, BomKind, LineEnding, detect_encoding, fix_lone_surrogates};
use super::unicode::{sha256_hex, find_non_ascii_chars, find_unmappable_char, ensure_crlf};

// ─── Types ───────────────────────────────────────────────────────────────────

pub struct SafeReadResult {
    /// 解码后的 Unicode 文本（孤立代理对已替换为 U+FFFD）
    pub content:     String,
    pub encoding:    EncodingInfo,
    /// sha256(原始字节)，用于 Edit 校验
    pub raw_hash:    String,
    /// sha256(LF 规范化后)，用于 CRLF 容错
    pub norm_hash:   String,
}

// ─── Safe read ───────────────────────────────────────────────────────────────

/// 安全读取：检测编码 → 解码 → 修复孤立代理对 → 计算双 hash
pub fn safe_read(path: &Path) -> Result<SafeReadResult, ToolError> {
    let raw = std::fs::read(path).map_err(|e| ToolError::IoError {
        message: e.to_string(),
    })?;

    let encoding = detect_encoding(&raw);

    let enc = Encoding::for_label(encoding.name.as_bytes())
        .unwrap_or(encoding_rs::UTF_8);

    let (cow, _, _) = enc.decode(&raw);
    let mut content = cow.into_owned();
    content = fix_lone_surrogates(content);

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

// ─── Safe write ──────────────────────────────────────────────────────────────

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
