// src/server/mod.rs — SeeyueMcpServer: struct, new(), ServerHandler, prompt_router

pub mod util;
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
                .enable_resources()
                .build()
        )
        .with_instructions(
            "seeyue-mcp: Windows-native file editing + workflow policy engine. \
             P0 Tools: read_file, write, edit, multi_edit, rewind — always read before edit/write. \
             P1 Hook Tools: sy_pretool_bash, sy_pretool_write, sy_posttool_write, sy_stop, \
             sy_create_checkpoint, sy_advance_node — call these for policy decisions. \
             P2 Prompts: skills registry via prompts/list and prompts/get. \
             P4 Extended: git_log, git_blame, batch_read, format_file, file_rename, \
             snapshot_workspace, call_hierarchy, compact_journal, search_session. \
             P3 Interactive: sy_notify, sy_approval_request/resolve/status, \
             sy_task_graph_update, sy_progress_report. \
             Resources: workflow://session, workflow://task-graph, workflow://journal, memory://index."
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
}
