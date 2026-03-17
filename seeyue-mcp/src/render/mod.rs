// src/render/mod.rs
//
// Output rendering: diff computation and formatting.

pub mod diff;

// Flat re-exports
#[allow(unused_imports)]
pub use diff::{compute_diff, DiffHunk, DiffLine, DiffLineKind, DiffResult, DiffSummary};
