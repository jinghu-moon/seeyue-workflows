// src/hooks/mod.rs
//
// Hook binary modules: protocol, routing, and event handlers.
// Used by the `sy-hook` binary to process Claude Code hook events.

pub mod protocol;
pub mod router;
pub mod session_start;
pub mod prompt_refresh;
pub mod posttool_bash;
pub mod verify_staging;
