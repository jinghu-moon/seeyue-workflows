// tests/test_encoding.rs
//
// Functional tests for encoding detection, safe read/write, and unicode utilities.
// Run: cargo test --test test_encoding

use std::io::Write;

use rstest::rstest;
use tempfile::NamedTempFile;

use seeyue_mcp::encoding::{
    detect_encoding, safe_read, sha256_hex, find_unicode_confusion, try_tab_normalized_match,
};

// ─── detect_encoding: BOM detection ──────────────────────────────────────────

#[rstest]
#[case(b"\xEF\xBB\xBFhello",  "UTF-8",    true,  1.0)]
#[case(b"\xFF\xFEh\x00i\x00", "UTF-16LE", true,  1.0)]
#[case(b"\xFE\xFF\x00h\x00i", "UTF-16BE", true,  1.0)]
fn test_bom_detection(
    #[case] input: &[u8],
    #[case] expected_name: &str,
    #[case] has_bom: bool,
    #[case] expected_confidence: f32,
) {
    let info = detect_encoding(input);
    assert_eq!(info.name, expected_name, "encoding name mismatch");
    assert_eq!(info.bom.is_some(), has_bom, "bom presence mismatch");
    assert!(
        (info.confidence - expected_confidence).abs() < 0.01,
        "confidence mismatch: got {}", info.confidence
    );
}

// ─── detect_encoding: ASCII fast-path ────────────────────────────────────────

#[test]
fn test_ascii_fastpath() {
    let input = b"fn main() { println!(\"hello\"); }";
    let info = detect_encoding(input);
    assert_eq!(info.name, "UTF-8");
    assert_eq!(info.bom, None);
    assert!(!info.has_non_ascii);
    assert!((info.confidence - 1.0).abs() < 0.01);
}

#[test]
fn test_non_ascii_detected() {
    let input = "// 你好世界".as_bytes();
    let info = detect_encoding(input);
    assert!(info.has_non_ascii);
}

// ─── detect_encoding: line ending detection ───────────────────────────────────

#[rstest]
#[case(b"line1\nline2\nline3",         "LF")]
#[case(b"line1\r\nline2\r\nline3",     "CRLF")]
#[case(b"line1\r\nline2\nline3",       "MIXED")]
#[case(b"no newlines here",            "LF")]   // default
fn test_line_ending_detection(#[case] input: &[u8], #[case] expected: &str) {
    let info = detect_encoding(input);
    let actual = format!("{:?}", info.line_ending);
    assert_eq!(actual.to_uppercase(), expected, "line ending mismatch");
}

// ─── sha256_hex ───────────────────────────────────────────────────────────────

#[test]
fn test_sha256_empty() {
    // SHA-256 of empty input is known
    let hash = sha256_hex(b"");
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_sha256_deterministic() {
    let data = b"hello world";
    assert_eq!(sha256_hex(data), sha256_hex(data));
}

#[test]
fn test_sha256_different_inputs() {
    assert_ne!(sha256_hex(b"hello"), sha256_hex(b"world"));
}

// ─── safe_read ────────────────────────────────────────────────────────────────

#[test]
fn test_safe_read_utf8_file() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"fn main() {}\n").unwrap();
    let result = safe_read(f.path()).unwrap();
    assert_eq!(result.content, "fn main() {}\n");
    assert_eq!(result.encoding.name, "UTF-8");
    assert!(!result.raw_hash.is_empty());
    assert!(!result.norm_hash.is_empty());
}

#[test]
fn test_safe_read_bom_utf8() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"\xEF\xBB\xBFhello\n").unwrap();
    let result = safe_read(f.path()).unwrap();
    assert_eq!(result.encoding.name, "UTF-8");
    assert!(result.encoding.bom.is_some());
    // BOM should be stripped from decoded content
    assert!(!result.content.starts_with('\u{FEFF}'));
}

#[test]
fn test_safe_read_crlf_norm_hash() {
    // raw_hash != norm_hash when file has CRLF
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"line1\r\nline2\r\n").unwrap();
    let result = safe_read(f.path()).unwrap();
    assert_ne!(
        result.raw_hash, result.norm_hash,
        "CRLF file should have different raw vs norm hash"
    );
}

#[test]
fn test_safe_read_lf_same_hash() {
    // LF file: raw_hash == norm_hash
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"line1\nline2\n").unwrap();
    let result = safe_read(f.path()).unwrap();
    assert_eq!(
        result.raw_hash, result.norm_hash,
        "LF-only file should have equal raw and norm hash"
    );
}

#[test]
fn test_safe_read_nonexistent() {
    use std::path::Path;
    let err = safe_read(Path::new("/nonexistent/path/file.rs"));
    assert!(err.is_err());
}

// ─── find_unicode_confusion ───────────────────────────────────────────────────

#[test]
fn test_unicode_confusion_detected() {
    // file has RIGHT SINGLE QUOTATION MARK (\u2019), query has ASCII apostrophe
    let file_content = "it\u{2019}s a trap";
    let old_string = "it's a trap";  // ASCII apostrophe
    let suggestions = find_unicode_confusion(old_string, file_content);
    assert!(
        !suggestions.is_empty(),
        "Should detect RIGHT SINGLE QUOTATION MARK vs apostrophe confusion"
    );
}

#[test]
fn test_unicode_confusion_clean() {
    let file_content = "it's simple";
    let old_string = "it's simple";
    let suggestions = find_unicode_confusion(old_string, file_content);
    assert!(suggestions.is_empty(), "No confusion in clean ASCII");
}

// ─── try_tab_normalized_match ─────────────────────────────────────────────────

#[rstest]
#[case("    fn foo()", "\tfn foo()", true)]   // spaces→tab
#[case("\tfn foo()", "    fn foo()", true)]   // tab→spaces
#[case("fn foo()", "fn foo()", false)]        // no change needed
fn test_tab_normalized_match(
    #[case] old_string: &str,
    #[case] file_content: &str,
    #[case] should_match: bool,
) {
    let result = try_tab_normalized_match(old_string, file_content);
    assert_eq!(
        result.is_some(), should_match,
        "tab normalization match mismatch for {:?}", old_string
    );
}
