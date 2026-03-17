// src/lib.rs
//
// Library facade: re-exports modules for integration tests and benchmarks.
// The binary entry point remains in main.rs.

pub mod encoding;
pub mod error;
pub mod hooks;
pub mod platform;
pub mod policy;
pub mod render;
pub mod storage;
pub mod treesitter;
pub mod workflow;
