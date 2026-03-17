// src/encoding/detect.rs
//
// Encoding detection: BOM, ASCII fast-path, chardetng.

use chardetng::EncodingDetector;
use serde::Serialize;

// ─── Types ───────────────────────────────────────────────────────────────────

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

// ─── Detection ───────────────────────────────────────────────────────────────

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
            line_ending:   LineEnding::Lf,
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
    let sample = if raw.len() > 4096 { &raw[..4096] } else { raw };
    let mut det = EncodingDetector::new();
    det.feed(sample, true);

    let encoding   = det.guess(None, true);
    let name       = encoding.name().to_string();
    let confidence = if name == "UTF-8" { 0.95 } else { 0.75 };

    EncodingInfo {
        name,
        confidence,
        bom:           None,
        has_non_ascii: raw.iter().any(|&b| b > 0x7F),
        line_ending:   detect_line_ending(raw),
    }
}

pub(super) fn detect_line_ending(raw: &[u8]) -> LineEnding {
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

pub(super) fn fix_lone_surrogates(s: String) -> String {
    s.chars()
        .map(|c| if c == '\u{FFFD}' { '\u{FFFD}' } else { c })
        .collect()
}
