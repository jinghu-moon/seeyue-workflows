//! interaction module — sy-interact presenter-only renderer.
//!
//! Sub-modules:
//!   schema     — DTOs aligned to workflow/interaction.schema.yaml
//!   io         — file read/write (atomic write_response)
//!   terminal   — terminal capability probing
//!   renderer   — text_menu + plain_prompt fallback renderers
//!   cli        — clap CLI definition and exit code constants
//!   state      — TUI focus/selection/comment state (P1)
//!   theme      — ratatui colour tokens (P1)
//!   render_tui — ratatui full TUI renderer (P1)

pub mod cli;
pub mod io;
pub mod render_tui;
pub mod renderer;
pub mod schema;
pub mod state;
pub mod terminal;
pub mod theme;
