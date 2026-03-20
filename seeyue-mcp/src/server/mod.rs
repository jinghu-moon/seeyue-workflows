// src/server/mod.rs — SeeyueMcpServer: struct, new(), ServerHandler, prompt_router

pub mod util;
pub mod schema;
pub mod tools_core;
pub mod tools_nav;
pub mod tools_git;
pub mod tools_mem;
pub mod tools_ia;
pub mod tools_ext;

use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{
        router::prompt::PromptRouter,
        router::tool::ToolRouter,
    },
    model::*,
    prompt_handler, prompt_router,
    tool_handler,
    service::RequestContext,
};

use crate::app_state::AppState;
use crate::prompts;
use crate::resources;

#[derive(Clone)]
pub struct SeeyueMcpServer {
    pub state:         AppState,
    pub tool_router:   ToolRouter<SeeyueMcpServer>,
    pub prompt_router: PromptRouter<SeeyueMcpServer>,
}

impl SeeyueMcpServer {
    pub fn new(state: AppState) -> Self {
        let mut prompt_router = Self::prompt_router();
        prompt_router.merge(prompts::build_prompt_router(state.skill_registry.clone()));
        Self {
            state,
            tool_router: Self::get_core_router()
                + Self::get_nav_router()
                + Self::get_git_router()
                + Self::get_mem_router()
                + Self::get_ia_router()
                + Self::get_ext_router(),
            prompt_router,
        }
    }
}

#[prompt_router]
impl SeeyueMcpServer {}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for SeeyueMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources_with(rmcp::model::ResourcesCapability {
                    subscribe:    Some(true),
                    list_changed: Some(true),
                })
                .enable_logging()
                .enable_completions()
                .build()
        )
        .with_instructions(
            "seeyue-mcp: Windows-native file editing + workflow policy engine. \
             PREFER sy_* tools over native Claude tools when working in this workspace. \
             P0 Tools: read_file, write, edit, multi_edit, rewind — always read before edit/write. \
             P1 Hook Tools: sy_pretool_bash, sy_pretool_write, sy_posttool_write, sy_stop, \
             sy_create_checkpoint, sy_advance_node — call these for policy decisions. \
             P2 Prompts: skills registry via prompts/list and prompts/get. \
             P4 Extended: git_log, git_blame, batch_read, format_file, file_rename, \
             snapshot_workspace, call_hierarchy, compact_journal, search_session. \
             P3 Interactive: sy_notify, sy_approval_request/resolve/status, \
             sy_task_graph_update, sy_progress_report. \
             Resources: workflow://session, workflow://task-graph, workflow://journal, memory://index. \
             Symbol Edit: replace_symbol_body, insert_after_symbol, insert_before_symbol — \
             PREFER over edit for whole-symbol changes."
            .to_string()
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: resources::workflow::list_resources(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        resources::workflow::read_resource(
            &request.uri,
            &self.state.workflow_dir,
            &self.state.workspace,
        )
        .map_err(|e| ErrorData::invalid_params(e, None))
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, ErrorData> {
        let prefix = request.argument.value.to_lowercase();
        let values: Vec<String> = match &request.r#ref {
            Reference::Prompt(prompt_ref) => {
                // Case 1: completing the skill name itself (argument name is common wildcards)
                if matches!(request.argument.name.as_str(), "$ARGUMENTS" | "skill" | "name" | "skill_name") {
                    self.state.skill_registry
                        .list_prompts()
                        .into_iter()
                        .map(|p| p.name)
                        .filter(|n| n.to_lowercase().starts_with(&prefix))
                        .take(20)
                        .collect()
                } else {
                    // Case 2: completing a specific argument value within a known prompt
                    let skill_name = &prompt_ref.name;
                    // Check if this skill has enumerated argument values
                    self.state.skill_registry
                        .entries()
                        .find(|e| &e.name == skill_name)
                        .and_then(|e| e.arguments.as_ref())
                        .and_then(|args| args.iter().find(|a| a.name == request.argument.name))
                        .and_then(|arg| arg.description.as_deref())
                        .map(|desc| {
                            // Parse pipe-separated enum hints from description: "one of: foo|bar|baz"
                            if let Some(after) = desc.split("one of:").nth(1) {
                                after.split('|')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty() && s.to_lowercase().starts_with(&prefix))
                                    .collect()
                            } else {
                                vec![]
                            }
                        })
                        .unwrap_or_else(|| {
                            // Case 3: built-in tool parameter enum table
                            builtin_param_completions(&request.argument.name, &prefix)
                        })
                }
            }
            Reference::Resource(_) => {
                // Complete known workflow resource URIs
                let known: &[&str] = &[
                    "workflow://session",
                    "workflow://task-graph",
                    "workflow://journal",
                    "workflow://questions",
                    "workflow://inputs",
                    "workflow://dashboard",
                    "memory://index",
                    "workspace://errors",
                ];
                known.iter()
                    .filter(|uri| uri.to_lowercase().starts_with(&prefix))
                    .map(|s| s.to_string())
                    .collect()
            }
        };
        let total = values.len() as u32;
        Ok(CompleteResult::new(CompletionInfo {
            values,
            total: Some(total),
            has_more: Some(false),
        }))
    }

    async fn set_level(
        &self,
        _request: SetLevelRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), rmcp::model::ErrorData> {
        // Accept logging/setLevel from clients (e.g. VS Code sends this before initialized).
        Ok(())
    }

    async fn subscribe(
        &self,
        _request: SubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), rmcp::model::ErrorData> {
        // Accept subscriptions — server pushes resource updates proactively via
        // notify_resource_updated on session/journal changes.
        Ok(())
    }

    async fn unsubscribe(
        &self,
        _request: UnsubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), rmcp::model::ErrorData> {
        Ok(())
    }
}

// ─── Completion Helpers ──────────────────────────────────────────────────────

/// Returns enum completions for well-known tool parameter names.
/// Used as a fallback when skill registry has no explicit enum hints.
fn builtin_param_completions(arg_name: &str, prefix: &str) -> Vec<String> {
    let candidates: &[&str] = match arg_name {
        "language" => &["rust", "python", "typescript", "javascript", "go", "java", "c", "cpp"],
        "linter"   => &["clippy", "eslint", "ruff", "auto"],
        "registry" => &["crates", "npm", "pypi"],
        "direction" => &["callers", "callees"],
        _ => &[],
    };
    candidates
        .iter()
        .filter(|v| v.starts_with(prefix))
        .map(|v| v.to_string())
        .collect()
}
