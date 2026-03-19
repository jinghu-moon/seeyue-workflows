// tests/interaction_migration.rs — P2-N3: Interaction Projection Tests
//
// Verifies that project_approval_as_interaction writes a valid
// interaction file under .ai/workflow/interactions/requests/.
// Also verifies ask_user and input_request projection functions.

use seeyue_mcp::tools::approval::project_approval_as_interaction;
use seeyue_mcp::tools::ask_user::project_ask_as_interaction;
use seeyue_mcp::tools::input_request::project_input_as_interaction;
use std::fs;
use tempfile::TempDir;

/// Helper: create a temp workflow dir with the expected layout.
fn temp_workflow_dir() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("create tempdir");
    let workflow_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&workflow_dir).expect("create workflow dir");
    (tmp, workflow_dir)
}

#[test]
fn test_approval_projection_creates_interaction_file() {
    let (_tmp, workflow_dir) = temp_workflow_dir();

    let result = project_approval_as_interaction(
        "ap-test-001",
        "Test approval subject",
        Some("Longer detail for the test"),
        Some("destructive"),
        &workflow_dir,
    )
    .expect("projection should succeed");

    // Assert file was created
    assert!(result.projected, "projected must be true");
    let file_path = std::path::Path::new(&result.file_path);
    assert!(file_path.exists(), "interaction file must exist at {}", result.file_path);

    // Read and parse the file
    let content = fs::read_to_string(file_path).expect("read interaction file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse interaction JSON");

    // Assert schema=1
    assert_eq!(obj["schema"], 1, "schema must be 1");

    // Assert kind=approval_request
    assert_eq!(
        obj["kind"].as_str().unwrap_or(""),
        "approval_request",
        "kind must be approval_request"
    );

    // Assert status=pending
    assert_eq!(
        obj["status"].as_str().unwrap_or(""),
        "pending",
        "status must be pending"
    );

    // Assert originating_request_id = approval_id
    assert_eq!(
        obj["originating_request_id"].as_str().unwrap_or(""),
        "ap-test-001",
        "originating_request_id must match approval_id"
    );

    // Assert interaction_id format: ix-YYYYMMDD-000
    let iid = obj["interaction_id"].as_str().unwrap_or("");
    assert!(
        iid.starts_with("ix-") && iid.len() >= 15,
        "interaction_id must match ix-YYYYMMDD-NNN format, got '{}'",
        iid
    );

    // Assert file is inside interactions/requests/
    let requests_dir = workflow_dir.join("interactions").join("requests");
    assert!(
        file_path.starts_with(&requests_dir),
        "file must be inside interactions/requests/"
    );
}

#[test]
fn test_approval_projection_no_detail() {
    let (_tmp, workflow_dir) = temp_workflow_dir();

    let result = project_approval_as_interaction(
        "ap-no-detail",
        "No detail subject",
        None,
        None,
        &workflow_dir,
    )
    .expect("projection without detail should succeed");

    assert!(result.projected);
    let content = fs::read_to_string(&result.file_path).expect("read file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
    assert_eq!(obj["schema"], 1);
    assert_eq!(obj["kind"].as_str().unwrap_or(""), "approval_request");
    // message should fall back to subject when detail is None
    assert_eq!(
        obj["message"].as_str().unwrap_or(""),
        "No detail subject",
        "message must fall back to subject when detail is None"
    );
}

#[test]
fn test_approval_projection_idempotent_overwrite() {
    // Writing the same approval_id twice should overwrite cleanly
    let (_tmp, workflow_dir) = temp_workflow_dir();

    project_approval_as_interaction(
        "ap-idem", "First write", None, None, &workflow_dir,
    ).expect("first write");

    let result = project_approval_as_interaction(
        "ap-idem", "Second write", Some("updated detail"), None, &workflow_dir,
    ).expect("second write");

    let content = fs::read_to_string(&result.file_path).expect("read file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
    // Second write should have updated title
    assert_eq!(
        obj["title"].as_str().unwrap_or(""),
        "Second write",
        "title must reflect second write"
    );
    assert_eq!(
        obj["message"].as_str().unwrap_or(""),
        "updated detail"
    );
}

// ─── ask_user projection ─────────────────────────────────────────────────────

#[test]
fn test_ask_user_projection_creates_interaction_file() {
    let tmp = TempDir::new().expect("create tempdir");
    let workflow_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&workflow_dir).expect("create workflow dir");

    let options = vec!["yes".to_string(), "no".to_string()];
    project_ask_as_interaction(
        "qst-test-001",
        "Do you want to continue?",
        Some(&options),
        Some("yes"),
        &workflow_dir,
    )
    .expect("ask projection should succeed");

    // Assert file created under interactions/requests/
    let file_path = workflow_dir
        .join("interactions")
        .join("requests")
        .join("qst-test-001.json");
    assert!(file_path.exists(), "ask interaction file must exist");

    let content = fs::read_to_string(&file_path).expect("read ask interaction file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

    assert_eq!(obj["schema"], 1);
    assert_eq!(obj["kind"].as_str().unwrap_or(""), "question_request",
        "kind must be question_request (canonical schema value)");
    assert_eq!(obj["status"].as_str().unwrap_or(""), "pending");
    assert_eq!(obj["originating_request_id"].as_str().unwrap_or(""), "qst-test-001");
    assert_eq!(obj["selection_mode"].as_str().unwrap_or(""), "single_select",
        "selection_mode must be single_select when options are provided");
    // interaction_id must match ^ix-[0-9]{8}-[0-9]{3,}$
    let ix_id = obj["interaction_id"].as_str().unwrap_or("");
    assert!(ix_id.starts_with("ix-"), "interaction_id must start with ix-");
    let parts: Vec<&str> = ix_id.split('-').collect();
    assert!(parts.len() >= 3, "interaction_id must have 3 parts");
    assert_eq!(parts[1].len(), 8, "date part must be 8 digits");
    assert!(parts[1].chars().all(|c| c.is_ascii_digit()), "date part must be digits");
    // Options array must contain the two entries
    let opts = obj["options"].as_array().expect("options must be array");
    assert_eq!(opts.len(), 2, "must have 2 options");
    // default_option_ids must map the default value
    let def_ids = obj["default_option_ids"].as_array().expect("default_option_ids must be array");
    assert_eq!(def_ids[0].as_str().unwrap_or(""), "yes");
}

#[test]
fn test_ask_user_projection_free_text_when_no_options() {
    let tmp = TempDir::new().expect("create tempdir");
    let workflow_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&workflow_dir).expect("create workflow dir");

    project_ask_as_interaction(
        "qst-free-001",
        "What is your name?",
        None,
        None,
        &workflow_dir,
    )
    .expect("ask projection should succeed");

    let file_path = workflow_dir
        .join("interactions")
        .join("requests")
        .join("qst-free-001.json");
    let content = fs::read_to_string(&file_path).expect("read file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

    assert_eq!(obj["selection_mode"].as_str().unwrap_or(""), "text",
        "selection_mode must be text when no options");
}

// ─── input_request projection ─────────────────────────────────────────────────

#[test]
fn test_input_request_projection_creates_interaction_file() {
    let tmp = TempDir::new().expect("create tempdir");
    let workflow_dir = tmp.path().join(".ai").join("workflow");
    fs::create_dir_all(&workflow_dir).expect("create workflow dir");

    project_input_as_interaction(
        "inp-test-001",
        "Enter the file path:",
        "file_path",
        None,
        Some("/src/main.rs"),
        &workflow_dir,
    )
    .expect("input projection should succeed");

    // Assert file created under interactions/requests/
    let file_path = workflow_dir
        .join("interactions")
        .join("requests")
        .join("inp-test-001.json");
    assert!(file_path.exists(), "input interaction file must exist");

    let content = fs::read_to_string(&file_path).expect("read input interaction file");
    let obj: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

    assert_eq!(obj["schema"], 1);
    assert_eq!(obj["kind"].as_str().unwrap_or(""), "input_request");
    assert_eq!(obj["status"].as_str().unwrap_or(""), "pending");
    assert_eq!(obj["originating_request_id"].as_str().unwrap_or(""), "inp-test-001");
    // file_path kind maps to selection_mode: path
    assert_eq!(obj["selection_mode"].as_str().unwrap_or(""), "path",
        "file_path kind must map to selection_mode=path");
    // example stored in detail field
    let detail = obj["detail"].as_str().unwrap_or("");
    assert!(detail.contains("/src/main.rs"), "example must appear in detail field");
    // interaction_id must match ^ix-[0-9]{8}-[0-9]{3,}$
    let ix_id = obj["interaction_id"].as_str().unwrap_or("");
    assert!(ix_id.starts_with("ix-"), "interaction_id must start with ix-");
}
