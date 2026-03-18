//! Integration tests: DTO model alignment with schema.

use seeyue_mcp::interaction::schema::{
    ColorProfile, CommentMode, InteractionKind, InteractionOption, InteractionRequest,
    InteractionStatus, PresentationHints, PresentationMode, ResponseStatus, Scope, SelectionMode,
    InteractionResponse,
};

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
        risk_level: None,
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

/// ResponseStatus serializes to schema-aligned snake_case values
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
    // Old names must NOT exist
    let json = serde_json::to_string(&ResponseStatus::Answered).unwrap();
    assert!(!json.contains("Completed"), "must not use old Completed variant");
    let json2 = serde_json::to_string(&ResponseStatus::Timeout).unwrap();
    assert!(!json2.contains("TimedOut"), "must not use old TimedOut variant");
}

/// scope field deserializes from object (not string)
#[test]
fn test_scope_is_object_not_string() {
    let json = serde_json::json!({
        "files": ["workflow/*.yaml"],
        "directories": ["workflow"]
    });
    let scope: Scope = serde_json::from_value(json).unwrap();
    assert_eq!(scope.files, Some(vec!["workflow/*.yaml".to_string()]));
    assert_eq!(scope.directories, Some(vec!["workflow".to_string()]));

    // Serializes back as object
    let serialized = serde_json::to_string(&scope).unwrap();
    assert!(serialized.starts_with('{'), "scope must be an object: {serialized}");

    // A plain string must NOT deserialize as Scope
    let result = serde_json::from_str::<Scope>("\"some-string\"");
    assert!(result.is_err(), "scope must not accept plain string");
}

/// response JSON has submitted_at and presenter.name
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
    // Old field names must be absent
    assert!(!json.contains("responded_at"), "old field responded_at must not appear");
    assert!(!json.contains("presenter_mode_used"), "old field must not appear");
}

/// answer_form field is present in serialized JSON
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
    assert!(json.contains("single_select"), "answer_form value must appear in JSON");
}

/// schema version is always 1
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
