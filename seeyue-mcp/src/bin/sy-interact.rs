//! sy-interact binary entry point.
//!
//! Delegates entirely to seeyue_mcp::interaction::cli::main().
//! This binary is a PRESENTER ONLY — no workflow logic, no policy decisions.
//!
//! See docs/sy-interact-cli-spec.md for the CLI contract.
//! See workflow/interaction.schema.yaml for the data schema.

use seeyue_mcp::interaction::cli;

fn main() {
    cli::main();
}
