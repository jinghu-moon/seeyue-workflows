// src/server/tools_ia.rs — P3 Interactive Tools

use rmcp::{tool, tool_router, handler::server::wrapper::Parameters, model::*, service::RequestContext, RoleServer};
use crate::params::*;
use crate::server::util::{to_text, tool_error_to_result};
use super::SeeyueMcpServer;

#[tool_router(router = ia_router)]
impl SeeyueMcpServer {
    #[tool(description = "Send a Windows Toast notification and record to journal. Levels: info | warn | milestone. Returns notified:bool and method used.")]
    async fn sy_notify(
        &self,
        Parameters(p): Parameters<SyNotifyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::notify::run_sy_notify(
            crate::tools::notify::SyNotifyParams {
                message:  p.message,
                level:    p.level,
                title:    p.title,
                progress: p.progress.map(|pp| crate::tools::notify::NotifyProgressParams {
                    value:  pp.value,
                    max:    pp.max,
                    label:  pp.label,
                    status: pp.status,
                }),
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Create an approval request. Uses MCP elicitation (inline popup) when supported; falls back to Toast + approvals.jsonl. Returns approval result with decision.")]
    async fn sy_approval_request(
        &self,
        Parameters(p): Parameters<ApprovalRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let subject      = p.subject.clone();
        let detail       = p.detail.clone();
        let category     = p.category.clone();
        let timeout_secs = p.timeout_secs;
        let workflow_dir = self.state.workflow_dir.clone();

        // Build elicitation message
        let message = match detail.as_deref() {
            Some(d) => format!("**{}**\n\n{}\n\nApprove this action?", subject, d),
            None    => format!("**{}**\n\nApprove this action?", subject),
        };

        // Try MCP elicitation first (requires client support)
        let schema = ElicitationSchema::builder()
            .required_bool("approved")
            .build_unchecked();

        let params = CreateElicitationRequestParams::FormElicitationParams {
            meta:             None,
            message,
            requested_schema: schema,
        };

        let timeout = timeout_secs
            .map(|s| std::time::Duration::from_secs(s));

        let approval_id = format!("apr_{}", chrono::Utc::now().timestamp_millis());

        match ctx.peer.create_elicitation_with_timeout(params, timeout).await {
            Ok(elicit_result) => {
                let decision = match elicit_result.action {
                    ElicitationAction::Accept => {
                        // Check the "approved" bool field
                        let approved = elicit_result.content
                            .as_ref()
                            .and_then(|v| v.get("approved"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        if approved { "approved" } else { "rejected" }
                    }
                    ElicitationAction::Decline  => "rejected",
                    ElicitationAction::Cancel   => "cancelled",
                };
                let result = crate::tools::approval::write_elicitation_resolved(
                    &approval_id,
                    &subject,
                    detail.as_deref(),
                    category.as_deref(),
                    decision,
                    &workflow_dir,
                );
                result.map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
            }
            Err(_) => {
                // Client does not support elicitation → try local presenter (sy-interact)
                let workspace    = self.state.workspace.clone();
                let workflow_dir2 = workflow_dir.clone();
                match crate::tools::approval::run_approval_via_presenter(
                    &approval_id,
                    &subject,
                    detail.as_deref(),
                    category.as_deref(),
                    timeout_secs,
                    &workspace,
                    &workflow_dir2,
                ) {
                    Ok(decision) => {
                        // Already written to approvals.jsonl by run_approval_via_presenter
                        let r = crate::tools::approval::ApprovalRequestResult {
                            kind:         decision.clone(),
                            approval_id:  approval_id.clone(),
                            subject:      subject.clone(),
                            status:       decision.clone(),
                            notified:     true,
                            timeout_secs,
                            expires_at:   None,
                        };
                        Ok(to_text(serde_json::to_string_pretty(&r).unwrap()))
                    }
                    Err(_) => {
                        // Presenter unavailable → fallback to Toast + jsonl
                        let result = crate::tools::approval::run_approval_request(
                            crate::tools::approval::ApprovalRequestParams {
                                subject,
                                detail,
                                category,
                                timeout_secs,
                            },
                            &workflow_dir,
                        );
                        result.map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
                    }
                }
            }
        }
    }

    #[tool(description = "Resolve a pending approval as approved or rejected. approval_id from sy_approval_request. decision: approved | rejected.")]
    async fn sy_approval_resolve(
        &self,
        Parameters(p): Parameters<ApprovalResolveParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::approval::run_approval_resolve(
            crate::tools::approval::ApprovalResolveParams {
                approval_id: p.approval_id,
                decision:    p.decision,
                note:        p.note,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Query approval status. If approval_id is omitted, returns all pending approvals.")]
    async fn sy_approval_status(
        &self,
        Parameters(p): Parameters<ApprovalStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::approval::run_approval_status(
            crate::tools::approval::ApprovalStatusParams {
                approval_id: p.approval_id,
                since_ts:    p.since_ts,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Update a node's status or notes in task-graph.yaml. status values: completed | in_progress | skipped | pending.")]
    async fn sy_task_graph_update(
        &self,
        Parameters(p): Parameters<TaskGraphUpdateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::task_graph_update::run_task_graph_update(
            crate::tools::task_graph_update::TaskGraphUpdateParams {
                node_id: p.node_id,
                status:  p.status,
                notes:   p.notes,
                nodes:   p.nodes.map(|ns| ns.into_iter().map(|n| crate::tools::task_graph_update::NodeUpdate {
                    node_id: n.node_id,
                    status:  n.status,
                    notes:   n.notes,
                }).collect()),
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Generate a human-readable progress report for the current workflow phase. Aggregates completed/total nodes, files written, key events.")]
    async fn sy_progress_report(
        &self,
        Parameters(p): Parameters<ProgressReportParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::progress_report::run_progress_report(
            crate::tools::progress_report::ProgressReportParams {
                phase:  p.phase,
                notify: p.notify.unwrap_or(false),
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Post a question to the user via Windows Toast + questions.jsonl. Returns question_id; poll sy_ask_user_status to retrieve the answer.")]
    async fn sy_ask_user(
        &self,
        Parameters(p): Parameters<AskUserParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::ask_user::run_ask_user(
            crate::tools::ask_user::AskUserParams {
                question: p.question,
                options:  p.options,
                default:  p.default,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Poll for user answers to questions posted by sy_ask_user. Omit question_id to return all pending questions.")]
    async fn sy_ask_user_status(
        &self,
        Parameters(p): Parameters<AskUserStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::ask_user::run_ask_user_status(
            crate::tools::ask_user::AskUserStatusParams {
                question_id: p.question_id,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Request structured input from the user (text/code/file_path/json) via Toast + input_requests.jsonl. Returns request_id.")]
    async fn sy_input_request(
        &self,
        Parameters(p): Parameters<InputRequestParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::input_request::run_input_request(
            crate::tools::input_request::InputRequestParams {
                prompt:   p.prompt,
                kind:     p.kind,
                language: p.language,
                example:  p.example,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Poll for submitted input responses from sy_input_request. Omit request_id to return all pending requests.")]
    async fn sy_input_status(
        &self,
        Parameters(p): Parameters<InputStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::input_request::run_input_status(
            crate::tools::input_request::InputStatusParams {
                request_id: p.request_id,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    // ─── P2-N4: Interaction MCP Tools ────────────────────────────────────────

    #[tool(description = "List pending interaction IDs from .ai/workflow/interactions/requests/. Filter by status (default: pending).")]
    async fn sy_list_interactions(
        &self,
        Parameters(p): Parameters<ListInteractionsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::interaction_mcp::run_list_interactions(
            crate::tools::interaction_mcp::ListInteractionsParams {
                status: p.status,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Read a specific interaction request by ID from .ai/workflow/interactions/requests/.")]
    async fn sy_read_interaction(
        &self,
        Parameters(p): Parameters<ReadInteractionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::interaction_mcp::run_read_interaction(
            crate::tools::interaction_mcp::ReadInteractionParams {
                interaction_id: p.interaction_id,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Probe interaction capability: checks for MCP elicitation support and sy-interact binary. Returns preferred_mode (elicitation|local_presenter|text_fallback).")]
    async fn sy_probe_interaction_capability(
        &self,
        Parameters(p): Parameters<ProbeInteractionCapabilityParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::interaction_strategy::run_probe_interaction_capability(
            crate::tools::interaction_strategy::ProbeInteractionCapabilityParams {
                workspace_override: p.workspace_override,
            },
            self.state.workspace.as_ref(),
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }

    #[tool(description = "Write a response file for an interaction (MCP-driven resolution). Writes to .ai/workflow/interactions/responses/.")]
    async fn sy_resolve_interaction(
        &self,
        Parameters(p): Parameters<ResolveInteractionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = crate::tools::interaction_mcp::run_resolve_interaction(
            crate::tools::interaction_mcp::ResolveInteractionParams {
                interaction_id:  p.interaction_id,
                selected_option: p.selected_option,
                comment:         p.comment,
            },
            &self.state.workflow_dir,
        );
        result
            .map_or_else(tool_error_to_result, |r| Ok(to_text(serde_json::to_string_pretty(&r).unwrap())))
    }
}

impl SeeyueMcpServer {
    pub(super) fn get_ia_router() -> rmcp::handler::server::router::tool::ToolRouter<SeeyueMcpServer> {
        Self::ia_router()
    }
}
