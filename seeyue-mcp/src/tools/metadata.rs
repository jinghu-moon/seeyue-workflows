// src/tools/metadata.rs
// ToolMetadata registry — single source of truth for tool capabilities.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    FileEdit,
    Git,
    Nav,
    Symbol,
    Workflow,
    Interact,
    Session,
    Exec,
    Ext,
}

#[derive(Debug, Clone)]
pub struct ToolMetadata {
    pub name:                 &'static str,
    pub description:          &'static str,
    pub category:             ToolCategory,
    pub read_only:            bool,
    pub destructive:          bool,
    pub mutates_durable_state: bool,
    pub requires_interaction: bool,
    pub requires_workspace:   bool,
    pub active_by_default:    bool,
}

// ─── Registry ────────────────────────────────────────────────────────────────

static TOOL_REGISTRY: OnceLock<HashMap<&'static str, ToolMetadata>> = OnceLock::new();

pub fn registry() -> &'static HashMap<&'static str, ToolMetadata> {
    TOOL_REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        register_all_tools(&mut m);
        m
    })
}

impl ToolMetadata {
    pub fn get(name: &str) -> Option<&'static ToolMetadata> {
        registry().get(name)
    }

    pub fn is_active(name: &str, active_tools: &HashSet<String>) -> bool {
        if active_tools.contains(name) {
            return true;
        }
        registry()
            .get(name)
            .map(|m| m.active_by_default)
            .unwrap_or(false)
    }
}

// ─── Registration ─────────────────────────────────────────────────────────────

fn reg(m: &mut HashMap<&'static str, ToolMetadata>, meta: ToolMetadata) {
    m.insert(meta.name, meta);
}

fn register_all_tools(m: &mut HashMap<&'static str, ToolMetadata>) {
    use ToolCategory::*;

    // File operations
    reg(m, ToolMetadata { name: "read_file", description: "Read file contents", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "write_file", description: "Write file contents", category: FileEdit, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "edit", description: "Edit file with old/new string replacement", category: FileEdit, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "multi_edit", description: "Apply multiple edits to a file", category: FileEdit, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "read_range", description: "Read specific line range of a file", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "read_compressed", description: "Read compressed file", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "file_rename", description: "Rename a file", category: FileEdit, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "file_outline", description: "Get file outline", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "batch_read", description: "Read multiple files", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "find_files", description: "Find files by pattern", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "preview_edit", description: "Preview an edit before applying", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "verify_syntax", description: "Verify file syntax", category: Exec, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "resolve_path", description: "Resolve a relative path", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "search_workspace", description: "Search workspace content", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "create_file_tree", description: "Create a file tree structure", category: FileEdit, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "snapshot_workspace", description: "Snapshot workspace state", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "workspace_tree", description: "Show workspace tree", category: FileEdit, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "env_info", description: "Get environment info", category: Ext, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: false, active_by_default: true });

    // Git
    reg(m, ToolMetadata { name: "git_log", description: "Show git log", category: Git, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "git_blame", description: "Show git blame", category: Git, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "git_status", description: "Show git status", category: Git, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "git_diff_file", description: "Show git diff for file", category: Git, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });

    // Nav / LSP
    reg(m, ToolMetadata { name: "go_to_definition", description: "Go to symbol definition", category: Nav, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "find_references", description: "Find all references to a symbol", category: Nav, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "get_hover_info", description: "Get hover information", category: Nav, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "call_hierarchy", description: "Get call hierarchy", category: Nav, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });

    // Symbol-first (new)
    reg(m, ToolMetadata { name: "sy_get_symbols_overview", description: "Get symbol tree for a file", category: Symbol, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_find_symbol", description: "Find symbol by name_path", category: Symbol, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_find_referencing_symbols", description: "Find symbols that reference a symbol", category: Symbol, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_replace_symbol_body", description: "Replace symbol body in-place", category: Symbol, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_insert_after_symbol", description: "Insert content after a symbol", category: Symbol, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_insert_before_symbol", description: "Insert content before a symbol", category: Symbol, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_rename_symbol", description: "Rename symbol across codebase", category: Symbol, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: false });

    // Workflow
    reg(m, ToolMetadata { name: "sy_advance_node", description: "Advance workflow node", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_create_checkpoint", description: "Create workflow checkpoint", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_stop", description: "Stop workflow session", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_session_start", description: "Start workflow session", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "diff_since_checkpoint", description: "Show diff since last checkpoint", category: Workflow, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "checkpoint_list", description: "List checkpoints", category: Workflow, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "tdd_evidence", description: "Record TDD evidence", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "session_summary", description: "Generate session summary", category: Workflow, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "task_graph_update", description: "Update task graph", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });

    // Interaction
    reg(m, ToolMetadata { name: "sy_approval_request", description: "Request human approval", category: Interact, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: true, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_approval_resolve", description: "Resolve approval request", category: Interact, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: true, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_approval_status", description: "Check approval status", category: Interact, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_ask_user", description: "Ask user a question", category: Interact, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: true, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_ask_user_status", description: "Check ask_user status", category: Interact, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_input_request", description: "Request user input", category: Interact, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: true, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_input_request_status", description: "Check input request status", category: Interact, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_notify", description: "Send notification", category: Interact, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: false, active_by_default: true });
    reg(m, ToolMetadata { name: "sy_progress_report", description: "Report progress", category: Interact, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });

    // System / Memory
    reg(m, ToolMetadata { name: "memory_read", description: "Read memory entry", category: Session, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "memory_write", description: "Write memory entry", category: Session, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "memory_delete", description: "Delete memory entry", category: Session, read_only: false, destructive: true, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "memory_list", description: "List memory entries", category: Session, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "compact_journal", description: "Compact workflow journal", category: Workflow, read_only: false, destructive: false, mutates_durable_state: true, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "search_session", description: "Search session journal", category: Workflow, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });

    // Lint / Quality
    reg(m, ToolMetadata { name: "lint_file", description: "Lint a file", category: Exec, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "type_check", description: "Type check source", category: Exec, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "format_file", description: "Format a file", category: Exec, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "dependency_graph", description: "Show dependency graph", category: Exec, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "symbol_rename_preview", description: "Preview symbol rename", category: Symbol, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "run_command", description: "Run a shell command", category: Exec, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "run_test", description: "Run tests", category: Exec, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "run_script", description: "Run a script", category: Exec, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "package_info", description: "Get package info", category: Ext, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: false, active_by_default: true });
    reg(m, ToolMetadata { name: "process_list", description: "List running processes", category: Ext, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: false, active_by_default: true });
    reg(m, ToolMetadata { name: "open_in_editor", description: "Open file in editor", category: Ext, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
    reg(m, ToolMetadata { name: "on_error", description: "Handle tool error", category: Ext, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: false, active_by_default: true });
    reg(m, ToolMetadata { name: "session_budget_warning", description: "Warn on session budget", category: Ext, read_only: true, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: false, active_by_default: true });
    reg(m, ToolMetadata { name: "multi_file_edit", description: "Edit multiple files", category: FileEdit, read_only: false, destructive: false, mutates_durable_state: false, requires_interaction: false, requires_workspace: true, active_by_default: true });
}
