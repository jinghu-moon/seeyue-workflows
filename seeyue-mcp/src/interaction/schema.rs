//! Schema-aligned DTOs for interaction requests and responses.
//!
//! Aligned with workflow/interaction.schema.yaml (schema_version: 1).
//! ResponseStatus variants match schema line 303 exactly.

use serde::{Deserialize, Serialize};

// ─── Shared enums ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    ApprovalRequest,
    RestoreRequest,
    QuestionRequest,
    InputRequest,
    ConflictResolution,
    HandoffNotice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionStatus {
    Pending,
    Answered,
    Cancelled,
    Expired,
    Failed,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    Boolean,
    SingleSelect,
    MultiSelect,
    Text,
    Number,
    Path,
    Secret,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentMode {
    Disabled,
    Optional,
    Required,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationMode {
    TuiMenu,
    TextMenu,
    PlainPrompt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorProfile {
    Auto,
    Mono,
    Ansi16,
    Ansi256,
    Rgb24,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Response status — aligned to schema line 303.
/// Answered (was Completed), Timeout (was TimedOut), Failed (was Error).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Answered,
    Cancelled,
    Timeout,
    Failed,
}

// ─── Scope struct — schema defines scope as object, not string ─────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,
}

// ─── Presenter info ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenterInfo {
    /// Always "sy-interact"
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub mode: PresentationMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_depth: Option<crate::interaction::terminal::ColorDepth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_kind: Option<String>,
}

// ─── Response analysis (P1 conflict detection — fields optional) ───────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseAnalysis {
    pub comment_conflicts_with_answer: bool,
    pub needs_clarification: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_reason: Option<String>,
}

// ─── Request DTOs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub recommended: bool,
    /// Visually flagged as dangerous (was: destructive — renamed to match schema)
    #[serde(default)]
    pub danger: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub requires_comment: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationHints {
    pub mode: PresentationMode,
    /// Required per schema line 212
    pub color_profile: ColorProfile,
    /// Required per schema line 213
    pub theme: String,
    /// Accent color token (e.g. "focus", "muted")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent_token: Option<String>,
    #[serde(default)]
    pub show_details_by_default: bool,
    #[serde(default = "default_true")]
    pub allow_alternate_screen: bool,
    /// Keymap overrides (freeform object)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keymap: Option<serde_json::Value>,
}

/// Origin of an interaction request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginSource {
    Runtime,
    HookClient,
    Mcp,
    Adapter,
    HostWrapper,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub source: OriginSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_id: Option<String>,
}

/// default_true helper for serde default.
fn default_true() -> bool { true }

/// Interaction request envelope — schema_version 1.
/// All fields and deny_unknown_fields aligned to workflow/interaction.schema.yaml.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRequest {
    /// Must be 1
    pub schema: u32,
    pub interaction_id: String,
    pub kind: InteractionKind,
    pub status: InteractionStatus,
    pub title: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<RiskLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocker_kind: Option<String>,
    pub selection_mode: SelectionMode,
    pub options: Vec<InteractionOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_option_ids: Option<Vec<String>>,
    pub comment_mode: CommentMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_placeholder: Option<String>,
    /// scope is an object per schema (not a string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_next: Option<Vec<serde_json::Value>>,
    pub presentation: PresentationHints,
    pub originating_request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,
}

// ─── Response DTOs ─────────────────────────────────────────────────────────

/// Interaction response envelope — schema_version 1.
/// Fields aligned to rework spec: answer_form, submitted_at, presenter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionResponse {
    /// Must be 1
    pub schema: u32,
    pub interaction_id: String,
    pub status: ResponseStatus,
    /// The selection_mode value as string (e.g. "single_select")
    pub answer_form: String,
    pub selected_option_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// RFC3339 timestamp (replaces responded_at)
    pub submitted_at: String,
    /// Presenter info (replaces presenter_mode_used: Option<String>)
    pub presenter: PresenterInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_analysis: Option<ResponseAnalysis>,
}

impl InteractionResponse {
    pub fn new(
        interaction_id: impl Into<String>,
        status: ResponseStatus,
        answer_form: impl Into<String>,
        mode: PresentationMode,
    ) -> Self {
        Self {
            schema: 1,
            interaction_id: interaction_id.into(),
            status,
            answer_form: answer_form.into(),
            selected_option_ids: vec![],
            comment: None,
            submitted_at: chrono::Utc::now().to_rfc3339(),
            presenter: PresenterInfo {
                name: "sy-interact".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                mode,
                color_depth: None,
                terminal_kind: None,
            },
            response_analysis: None,
        }
    }
}

// ─── Validation ────────────────────────────────────────────────────────────

/// Validate an InteractionRequest against schema rules.
/// Returns Err with human-readable message if validation fails.
pub fn validate_request(req: &InteractionRequest) -> Result<(), String> {
    if req.schema != 1 {
        return Err(format!("schema must be 1, got {}", req.schema));
    }
    // interaction_id pattern: ^ix-[0-9]{8}-[0-9]{3,}$
    let id = &req.interaction_id;
    if !is_valid_interaction_id(id) {
        return Err(format!(
            "interaction_id '{}' does not match pattern ^ix-[0-9]{{8}}-[0-9]{{3,}}$",
            id
        ));
    }
    if req.title.is_empty() {
        return Err("title is required and must not be empty".to_string());
    }
    if req.message.is_empty() {
        return Err("message is required and must not be empty".to_string());
    }
    if req.options.is_empty() {
        return Err("options must not be empty".to_string());
    }
    if req.originating_request_id.is_empty() {
        return Err("originating_request_id is required".to_string());
    }
    if req.created_at.is_empty() {
        return Err("created_at is required".to_string());
    }
    if req.presentation.theme.is_empty() {
        return Err("presentation.theme is required and must not be empty".to_string());
    }
    Ok(())
}

fn is_valid_interaction_id(id: &str) -> bool {
    // ^ix-[0-9]{8}-[0-9]{3,}$
    if !id.starts_with("ix-") {
        return false;
    }
    let rest = &id[3..];
    let parts: Vec<&str> = rest.splitn(2, '-').collect();
    if parts.len() != 2 {
        return false;
    }
    let date_part = parts[0];
    let seq_part = parts[1];
    date_part.len() == 8
        && date_part.chars().all(|c| c.is_ascii_digit())
        && seq_part.len() >= 3
        && seq_part.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> InteractionRequest {
        InteractionRequest {
            schema: 1,
            interaction_id: "ix-20260318-001".to_string(),
            kind: InteractionKind::ApprovalRequest,
            status: InteractionStatus::Pending,
            title: "高风险写入确认".to_string(),
            message: "即将覆盖 workflow/*.yaml".to_string(),
            selection_mode: SelectionMode::SingleSelect,
            options: vec![InteractionOption {
                id: "approve_once".to_string(),
                label: "确认继续".to_string(),
                description: None,
                recommended: true,
                danger: false,
                disabled: false,
                requires_comment: false,
                metadata: None,
            }],
            comment_mode: CommentMode::Optional,
            presentation: PresentationHints {
                mode: PresentationMode::TextMenu,
                color_profile: ColorProfile::Auto,
                theme: "auto".to_string(),
                accent_token: None,
                show_details_by_default: false,
                allow_alternate_screen: true,
                keymap: None,
            },
            originating_request_id: "req-123".to_string(),
            created_at: "2026-03-18T00:00:00Z".to_string(),
            detail: None,
            reason_code: None,
            risk_level: Some(RiskLevel::High),
            blocker_kind: None,
            default_option_ids: None,
            comment_label: None,
            comment_placeholder: None,
            recommended_next: None,
            origin: None,
            expires_at: None,
            timeout_seconds: None,
            scope: None,
        }
    }

    #[test]
    fn test_response_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Answered).unwrap(),
            "\"answered\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Timeout).unwrap(),
            "\"timeout\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&ResponseStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn test_scope_is_object_not_string() {
        let scope = Scope {
            files: Some(vec!["workflow/*.yaml".to_string()]),
            commands: None,
            directories: Some(vec!["workflow/".to_string()]),
            services: None,
        };
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.starts_with('{'), "scope must serialize as object, got: {json}");
        assert!(json.contains("files"));
        assert!(json.contains("directories"));
        let decoded: Scope = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.directories, Some(vec!["workflow/".to_string()]));
    }

    #[test]
    fn test_response_has_submitted_at_and_presenter() {
        let resp = InteractionResponse::new(
            "ix-20260318-001",
            ResponseStatus::Answered,
            "single_select",
            PresentationMode::TextMenu,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("submitted_at"), "must have submitted_at");
        assert!(json.contains("presenter"), "must have presenter");
        assert!(json.contains("sy-interact"), "presenter.name must be sy-interact");
        assert!(!json.contains("responded_at"), "old field must not appear");
        assert!(!json.contains("presenter_mode_used"), "old field must not appear");
    }

    #[test]
    fn test_response_has_answer_form() {
        let resp = InteractionResponse::new(
            "ix-20260318-002",
            ResponseStatus::Answered,
            "single_select",
            PresentationMode::PlainPrompt,
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("answer_form"), "must have answer_form field");
        assert!(json.contains("single_select"), "answer_form value must be present");
    }

    #[test]
    fn test_schema_version_is_1() {
        let req = sample_request();
        assert_eq!(req.schema, 1);
        let resp = InteractionResponse::new(
            "ix-test-001",
            ResponseStatus::Cancelled,
            "single_select",
            PresentationMode::PlainPrompt,
        );
        assert_eq!(resp.schema, 1);
    }

    #[test]
    fn test_validate_request_ok() {
        let req = sample_request();
        assert!(validate_request(&req).is_ok());
    }

    #[test]
    fn test_validate_request_bad_id() {
        let mut req = sample_request();
        req.interaction_id = "bad-id".to_string();
        assert!(validate_request(&req).is_err());
    }

    #[test]
    fn test_validate_request_empty_title() {
        let mut req = sample_request();
        req.title = String::new();
        let err = validate_request(&req).unwrap_err();
        assert!(err.contains("title"));
    }

    #[test]
    fn test_interaction_id_pattern() {
        assert!(is_valid_interaction_id("ix-20260318-001"));
        assert!(is_valid_interaction_id("ix-20260318-1234"));
        assert!(!is_valid_interaction_id("bad-id"));
        assert!(!is_valid_interaction_id("ix-2026-001"));
        assert!(!is_valid_interaction_id("ix-20260318-01")); // seq too short
    }
}
