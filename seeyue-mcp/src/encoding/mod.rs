// src/encoding/mod.rs
//
// Encoding module: detection, safe read/write, and Unicode utilities.
// Re-exports all public symbols to maintain the same interface as the
// previous single-file encoding.rs.

pub mod detect;
pub mod rw;
pub mod unicode;

// ─── Flat re-exports (backward-compatible public API) ────────────────────────

#[allow(unused_imports)]
pub use detect::{BomKind, EncodingInfo, LineEnding, detect_encoding};
#[allow(unused_imports)]
pub use rw::{SafeReadResult, safe_read, safe_write};
pub use unicode::{sha256_hex, find_unicode_confusion, try_tab_normalized_match};
