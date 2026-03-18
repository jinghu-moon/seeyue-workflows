//! interaction module — sy-interact presenter-only renderer.
//!
//! Sub-modules:
//!   schema   — DTOs aligned to workflow/interaction.schema.yaml
//!   io       — file read/write (atomic write_response)
//!   terminal — terminal capability probing (no ratatui)
//!   renderer — text_menu + plain_prompt fallback renderers
//!   cli      — clap CLI definition and exit code constants

pub mod cli;
pub mod io;
pub mod renderer;
pub mod schema;
pub mod terminal;
