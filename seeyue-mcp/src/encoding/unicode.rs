// src/encoding/unicode.rs
//
// Unicode utilities: confusion detection, tab normalization, sha256, CRLF.

use encoding_rs::Encoding;
use sha2::{Digest, Sha256};

use crate::error::{CharInfo, NonAsciiChar, Suggestion, unicode_char_name};

// ─── Public API ──────────────────────────────────────────────────────────────

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// Unicode 常见混淆对检测（根因 E4）
pub fn find_unicode_confusion(
    old_string: &str,
    file_content: &str,
) -> Vec<Suggestion> {
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
        let file_has    = file_content.contains(file_char);
        let query_has   = old_string.contains(query_char);
        let query_lacks = !old_string.contains(file_char);

        if file_has && query_has && query_lacks {
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
    let with_tabs = old_string.replace("    ", "\t");
    if with_tabs != old_string && file_content.contains(&with_tabs) {
        return Some(with_tabs);
    }
    let with_spaces = old_string.replace('\t', "    ");
    if with_spaces != old_string && file_content.contains(&with_spaces) {
        return Some(with_spaces);
    }
    let with_tabs_2 = old_string.replace("  ", "\t");
    if with_tabs_2 != old_string && file_content.contains(&with_tabs_2) {
        return Some(with_tabs_2);
    }
    None
}

// ─── Internal helpers (used by rw.rs) ────────────────────────────────────────

pub(super) fn find_non_ascii_chars(s: &str) -> Vec<NonAsciiChar> {
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

pub(super) fn find_unmappable_char(
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

pub(super) fn ensure_crlf(bytes: Vec<u8>) -> Vec<u8> {
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
