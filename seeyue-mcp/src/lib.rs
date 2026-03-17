// src/lib.rs
//
// Library facade: re-exports modules for integration tests and benchmarks.
// The binary entry point remains in main.rs.

pub mod app_state;
pub mod encoding;
pub mod error;
pub mod git;
pub mod hooks;
pub mod lsp;
pub mod platform;
pub mod policy;
pub mod prompts;
pub mod render;
pub mod storage;
pub mod treesitter;
pub mod workflow;

pub mod tools {
    pub mod batch_read;
    pub mod call_hierarchy;
    pub mod compact_journal;
    pub mod create_file_tree;
    pub mod dependency_graph;
    pub mod diff_since_checkpoint;
    pub mod edit;
    pub mod env_info;
    pub mod file_outline;
    pub mod file_rename;
    pub mod find_definition;
    pub mod find_references;
    pub mod format_file;
    pub mod git_blame;
    pub mod git_diff_file;
    pub mod git_log;
    pub mod git_status;
    pub mod lint_file;
    pub mod multi_file_edit;
    pub mod package_info;
    pub mod preview_edit;
    pub mod read;
    pub mod read_compressed;
    pub mod read_range;
    pub mod resolve_path;
    pub mod run_command;
    pub mod run_test;
    pub mod search_session;
    pub mod search_workspace;
    pub mod session_summary;
    pub mod snapshot_workspace;
    pub mod symbol_rename_preview;
    pub mod type_check;
    pub mod verify_syntax;
    pub mod workspace_tree;
    pub mod write;
}
