//! File I/O for interaction request and response files.
//!
//! Requests and responses are stored as JSON files under .ai/workflow/interactions/.
//! See docs/interaction-runtime-integration.md for the directory layout.
//! write_response uses atomic rename to avoid partial writes.

use std::path::Path;
use crate::interaction::schema::{InteractionRequest, InteractionResponse};

/// Read an InteractionRequest from a JSON file.
pub fn read_request(path: &Path) -> Result<InteractionRequest, IoError> {
    let content = std::fs::read_to_string(path).map_err(|e| IoError::Io {
        path: path.to_string_lossy().into_owned(),
        message: e.to_string(),
    })?;
    serde_json::from_str(&content).map_err(|e| IoError::Parse {
        path: path.to_string_lossy().into_owned(),
        message: e.to_string(),
    })
}

/// Write an InteractionResponse to a JSON file, atomically.
/// Creates parent directories automatically.
/// Uses write-to-tmp then rename to avoid partial writes.
pub fn write_response(path: &Path, response: &InteractionResponse) -> Result<(), IoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| IoError::Io {
            path: parent.to_string_lossy().into_owned(),
            message: e.to_string(),
        })?;
    }
    let content = serde_json::to_string_pretty(response).map_err(|e| IoError::Serialize {
        message: e.to_string(),
    })?;
    // Atomic: write to .tmp then rename
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &content).map_err(|e| IoError::Io {
        path: tmp_path.to_string_lossy().into_owned(),
        message: e.to_string(),
    })?;
    std::fs::rename(&tmp_path, path).map_err(|e| IoError::Io {
        path: path.to_string_lossy().into_owned(),
        message: format!("rename failed: {}", e),
    })
}

/// Errors from file I/O operations.
#[derive(Debug)]
pub enum IoError {
    Io { path: String, message: String },
    Parse { path: String, message: String },
    Serialize { message: String },
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::Io { path, message } => write!(f, "IO error at {path}: {message}"),
            IoError::Parse { path, message } => write!(f, "Parse error at {path}: {message}"),
            IoError::Serialize { message } => write!(f, "Serialize error: {message}"),
        }
    }
}

impl std::error::Error for IoError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::schema::{
        CommentMode, InteractionKind, InteractionOption, InteractionRequest, InteractionResponse,
        InteractionStatus, PresentationHints, PresentationMode, ResponseStatus, SelectionMode,
    };
    use tempfile::tempdir;

    fn make_request() -> InteractionRequest {
        InteractionRequest {
            schema: 1,
            interaction_id: "ix-20260318-001".to_string(),
            kind: InteractionKind::ApprovalRequest,
            status: InteractionStatus::Pending,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            selection_mode: SelectionMode::SingleSelect,
            options: vec![InteractionOption {
                id: "ok".to_string(),
                label: "OK".to_string(),
                description: None,
                recommended: true,
                danger: false,
                disabled: false,
                requires_comment: false,
                metadata: None,
            }],
            comment_mode: CommentMode::Disabled,
            presentation: PresentationHints {
                mode: PresentationMode::TextMenu,
                color_profile: crate::interaction::schema::ColorProfile::Auto,
                theme: "auto".to_string(),
                accent_token: None,
                show_details_by_default: false,
                allow_alternate_screen: true,
                keymap: None,
            },
            originating_request_id: "req-1".to_string(),
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

    #[test]
    fn test_read_request_roundtrip() {
        let dir = tempdir().unwrap();
        let req_path = dir.path().join("ix-20260318-001.json");
        let req = make_request();
        let json = serde_json::to_string_pretty(&req).unwrap();
        std::fs::write(&req_path, &json).unwrap();

        let loaded = read_request(&req_path).expect("read_request failed");
        assert_eq!(loaded.interaction_id, req.interaction_id);
        assert_eq!(loaded.schema, 1);
        assert_eq!(loaded.options.len(), 1);
    }

    #[test]
    fn test_read_request_missing_file_returns_error() {
        let result = read_request(Path::new("/nonexistent/ix-missing.json"));
        assert!(result.is_err(), "should fail on missing file");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("IO error") || msg.contains("error"),
            "error message: {msg}"
        );
    }

    #[test]
    fn test_write_response_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("resp.json");
        let resp = InteractionResponse::new(
            "ix-test-003",
            ResponseStatus::Cancelled,
            "single_select",
            PresentationMode::PlainPrompt,
        );
        write_response(&nested, &resp).expect("write_response with nested dirs failed");
        assert!(nested.exists());
    }

    #[test]
    fn test_write_response_atomic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("resp.json");
        let resp = InteractionResponse::new(
            "ix-test-004",
            ResponseStatus::Answered,
            "single_select",
            PresentationMode::TextMenu,
        );
        write_response(&path, &resp).expect("write_response failed");
        assert!(path.exists());
        // .tmp must not remain after successful write
        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), ".tmp file must be cleaned up after rename");
    }

    #[test]
    fn test_write_response_content_is_valid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("resp.json");
        let resp = InteractionResponse::new(
            "ix-test-005",
            ResponseStatus::Answered,
            "single_select",
            PresentationMode::TextMenu,
        );
        write_response(&path, &resp).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["schema"], 1);
        assert_eq!(parsed["interaction_id"], "ix-test-005");
        assert!(parsed.get("submitted_at").is_some());
        assert!(parsed.get("presenter").is_some());
        assert!(parsed.get("answer_form").is_some());
    }
}
