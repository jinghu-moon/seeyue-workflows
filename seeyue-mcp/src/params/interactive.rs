// src/params/interactive.rs — P3 Interactive + Ask/Input tool params

use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SyNotifyParams {
    #[schemars(description = "Notification body message")]
    pub message:  String,
    #[schemars(description = "Level: info (default) | warn | milestone")]
    pub level:    Option<String>,
    #[schemars(description = "Optional title override (default: seeyue-mcp)")]
    pub title:    Option<String>,
    #[schemars(description = "Optional progress bar (value 0.0-1.0, negative=indeterminate)")]
    pub progress: Option<NotifyProgressParams>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NotifyProgressParams {
    #[schemars(description = "Progress value 0.0-1.0; negative = indeterminate")]
    pub value:  f32,
    #[schemars(description = "Denominator label (e.g. 100)")]
    pub max:    Option<String>,
    #[schemars(description = "Label above bar")]
    pub label:  Option<String>,
    #[schemars(description = "Status text below bar")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ApprovalRequestParams {
    #[schemars(description = "Short subject line shown in the toast and approval list")]
    pub subject:      String,
    #[schemars(description = "Optional longer description")]
    pub detail:       Option<String>,
    #[schemars(description = "Category tag (e.g. destructive, deploy, policy)")]
    pub category:     Option<String>,
    #[schemars(description = "Auto-reject after this many seconds if not resolved (omit = no timeout)")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ApprovalResolveParams {
    #[schemars(description = "Approval ID returned by sy_approval_request")]
    pub approval_id: String,
    #[schemars(description = "Decision: approved | rejected")]
    pub decision:    String,
    #[schemars(description = "Optional note recorded with the resolution")]
    pub note:        Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct ApprovalStatusParams {
    #[schemars(description = "If provided, fetch a specific approval. Otherwise returns all pending.")]
    pub approval_id: Option<String>,
    #[schemars(description = "Return only entries created at or after this ISO 8601 timestamp")]
    pub since_ts:    Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NodeUpdateItem {
    #[schemars(description = "Node id")]
    pub node_id: String,
    #[schemars(description = "New status value")]
    pub status:  Option<String>,
    #[schemars(description = "Notes to attach")]
    pub notes:   Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct TaskGraphUpdateParams {
    #[schemars(description = "Node id to update (single-node mode)")]
    pub node_id: Option<String>,
    #[schemars(description = "New status value (e.g. completed, in_progress, skipped)")]
    pub status:  Option<String>,
    #[schemars(description = "Notes to attach to the node")]
    pub notes:   Option<String>,
    #[schemars(description = "Batch mode: list of {node_id, status?, notes?} updates")]
    pub nodes:   Option<Vec<NodeUpdateItem>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct ProgressReportParams {
    #[schemars(description = "Filter to a specific phase id/name (default: current phase)")]
    pub phase:  Option<String>,
    #[schemars(description = "Send a Windows Toast with the summary line (default: false)")]
    pub notify: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AskUserParams {
    #[schemars(description = "The question to present to the user")]
    pub question: String,
    #[schemars(description = "Optional list of valid choices")]
    pub options:  Option<Vec<String>>,
    #[schemars(description = "Default answer hint")]
    pub default:  Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct AskUserStatusParams {
    #[schemars(description = "Question ID returned by sy_ask_user (omit = all pending)")]
    pub question_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InputRequestParams {
    #[schemars(description = "Short description of what is needed")]
    pub prompt:   String,
    #[schemars(description = "Input kind: text | code | file_path | json (default: text)")]
    pub kind:     Option<String>,
    #[schemars(description = "Language hint when kind==code (e.g. rust)")]
    pub language: Option<String>,
    #[schemars(description = "Optional example value shown to the user")]
    pub example:  Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub struct InputStatusParams {
    #[schemars(description = "Request ID returned by sy_input_request (omit = all pending)")]
    pub request_id: Option<String>,
}
