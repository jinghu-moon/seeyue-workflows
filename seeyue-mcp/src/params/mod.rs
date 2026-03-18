// src/params/mod.rs
//
// MCP tool parameter structs for SeeyueMcpServer.
// Organized by functional group; all types are re-exported flat
// so main.rs can continue to use `use params::*`.

pub mod editing;
pub mod extended;
pub mod execution;
pub mod git;
pub mod interactive;
pub mod memory;
pub mod navigation;
pub mod platform;

// Flat re-exports — main.rs uses `use params::*`
pub use editing::*;
pub use extended::*;
pub use execution::*;
pub use git::*;
pub use interactive::*;
pub use memory::*;
pub use navigation::*;
pub use platform::*;
