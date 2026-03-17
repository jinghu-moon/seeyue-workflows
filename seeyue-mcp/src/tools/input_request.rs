// src/tools/input_request.rs
//
// sy_input_request: Request structured input from the user (code, path, text).
// sy_input_status:  Poll for submitted input.
//
// Persists to .ai/workflow/input_requests.jsonl.
// User fills in the `response` field manually or via a companion UI.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::journal::{self, JournalEvent};

const INPUT_FILE: &str = "input_requests.jsonl";

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InputRequestParams {
    /// Short description of what is needed.
    pub prompt:       String,
    /// Input kind hint: "text" | "code" | "file_path" | "json" (default: "text").
    pub kind:         Option<String>,
    /// Language hint when kind=="code" (e.g. "rust", "python").
    pub language:     Option<String>,
    /// Optional example value shown to the user.
    pub example:      Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct InputStatusParams {
    /// Input request ID returned by sy_input_request.
    pub request_id: Option<String>,
}

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InputRequestResult {
    #[serde(rename = "type")]
    pub kind:       String, // "pending"
    pub request_id: String,
    pub prompt:     String,
    pub notified:   bool,
}

#[derive(Debug, Serialize)]
pub struct InputEntry {
    pub request_id:   String,
    pub prompt:       String,
    pub kind:         String,
    pub language:     Option<String>,
    pub example:      Option<String>,
    pub status:       String, // "pending" | "submitted"
    pub response:     Option<String>,
    pub ts:           String,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InputStatusResult {
    #[serde(rename = "type")]
    pub kind:     String, // "submitted" | "pending" | "empty"
    pub total:    usize,
    pub requests: Vec<InputEntry>,
}

// ─── Internal record ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct InputRecord {
    request_id:   String,
    ts:           String,
    prompt:       String,
    kind:         String,
    language:     Option<String>,
    example:      Option<String>,
    status:       String,
    response:     Option<String>,
    submitted_at: Option<String>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn input_path(workflow_dir: &Path) -> std::path::PathBuf {
    workflow_dir.join(INPUT_FILE)
}

fn load_requests(workflow_dir: &Path) -> Vec<InputRecord> {
    let path = input_path(workflow_dir);
    if !path.exists() { return vec![]; }
    let content = fs::read_to_string(&path).unwrap_or_default();
    let mut map: std::collections::HashMap<String, InputRecord> =
        std::collections::HashMap::new();
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        if let Ok(r) = serde_json::from_str::<InputRecord>(line) {
            map.insert(r.request_id.clone(), r);
        }
    }
    let mut records: Vec<InputRecord> = map.into_values().collect();
    records.sort_by(|a, b| b.ts.cmp(&a.ts));
    records
}

fn append_record(workflow_dir: &Path, record: &InputRecord) -> Result<(), ToolError> {
    let path = input_path(workflow_dir);
    let line = serde_json::to_string(record)
        .map_err(|e| ToolError::IoError { message: format!("serialize input_request: {e}") })?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| ToolError::IoError { message: format!("open input_requests: {e}") })?;
    writeln!(file, "{}", line)
        .map_err(|e| ToolError::IoError { message: format!("write input_request: {e}") })?;
    Ok(())
}

// ─── sy_input_request ────────────────────────────────────────────────────────

pub fn run_input_request(
    params: InputRequestParams,
    workflow_dir: &Path,
) -> Result<InputRequestResult, ToolError> {
    if params.prompt.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "prompt".into(),
            hint:    "Provide a non-empty input prompt.".into(),
        });
    }

    let request_id = format!("inp_{}", Utc::now().timestamp_millis());
    let ts         = Utc::now().to_rfc3339();
    let kind_str   = params.kind.clone().unwrap_or_else(|| "text".into());

    let record = InputRecord {
        request_id:   request_id.clone(),
        ts:           ts.clone(),
        prompt:       params.prompt.clone(),
        kind:         kind_str.clone(),
        language:     params.language.clone(),
        example:      params.example.clone(),
        status:       "pending".into(),
        response:     None,
        submitted_at: None,
    };
    append_record(workflow_dir, &record)?;

    // Toast
    let kind_hint = match kind_str.as_str() {
        "code"      => " [code input needed]",
        "file_path" => " [file path needed]",
        "json"      => " [JSON input needed]",
        _           => " [text input needed]",
    };
    let toast_body = format!("{}{}", params.prompt, kind_hint);
    let toast = win_notify::send_toast(
        "seeyue-mcp [input]",
        &toast_body,
        NotifyLevel::Warn,
    );

    // Journal
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "input_requested".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "request_id": request_id,
            "prompt":     params.prompt,
            "kind":       kind_str,
        })),
        phase:    None,
        node_id:  None,
        run_id:   None,
        ts:       Utc::now().to_rfc3339(),
        trace_id: None,
    });

    Ok(InputRequestResult {
        kind:       "pending".into(),
        request_id,
        prompt:     params.prompt,
        notified:   toast.notified,
    })
}

// ─── sy_input_status ─────────────────────────────────────────────────────────

pub fn run_input_status(
    params: InputStatusParams,
    workflow_dir: &Path,
) -> Result<InputStatusResult, ToolError> {
    let all = load_requests(workflow_dir);

    let filtered: Vec<InputEntry> = all.iter()
        .filter(|r| {
            if let Some(ref id) = params.request_id {
                &r.request_id == id
            } else {
                r.status == "pending"
            }
        })
        .map(|r| InputEntry {
            request_id:   r.request_id.clone(),
            prompt:       r.prompt.clone(),
            kind:         r.kind.clone(),
            language:     r.language.clone(),
            example:      r.example.clone(),
            status:       r.status.clone(),
            response:     r.response.clone(),
            ts:           r.ts.clone(),
            submitted_at: r.submitted_at.clone(),
        })
        .collect();

    let submitted = filtered.iter().filter(|e| e.status == "submitted").count();
    let kind = if filtered.is_empty() {
        "empty"
    } else if submitted == filtered.len() {
        "submitted"
    } else {
        "pending"
    };

    Ok(InputStatusResult {
        kind:     kind.into(),
        total:    filtered.len(),
        requests: filtered,
    })
}
