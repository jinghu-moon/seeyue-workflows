// src/tools/ask_user.rs
//
// sy_ask_user:        Post a question to the user via Toast + questions.jsonl.
// sy_ask_user_status: Poll for the answer written by the user.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::platform::notify::{self as win_notify, NotifyLevel};
use crate::workflow::journal::{self, JournalEvent};

const QUESTIONS_FILE: &str = "questions.jsonl";

// ─── Params ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AskUserParams {
    /// The question to present to the user.
    pub question: String,
    /// Optional list of valid choices (shown in toast body).
    pub options:  Option<Vec<String>>,
    /// Default answer if user does not respond (used as hint only; not auto-applied).
    pub default:  Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AskUserStatusParams {
    /// Question ID returned by sy_ask_user.
    pub question_id: Option<String>,
}

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AskUserResult {
    #[serde(rename = "type")]
    pub kind:        String, // "pending"
    pub question_id: String,
    pub question:    String,
    pub notified:    bool,
}

#[derive(Debug, Serialize)]
pub struct QuestionEntry {
    pub question_id: String,
    pub question:    String,
    pub options:     Option<Vec<String>>,
    pub default:     Option<String>,
    pub status:      String, // "pending" | "answered"
    pub answer:      Option<String>,
    pub ts:          String,
    pub answered_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AskUserStatusResult {
    #[serde(rename = "type")]
    pub kind:      String, // "answered" | "pending" | "empty"
    pub total:     usize,
    pub questions: Vec<QuestionEntry>,
}

// ─── Internal record ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct QuestionRecord {
    question_id: String,
    ts:          String,
    question:    String,
    options:     Option<Vec<String>>,
    default:     Option<String>,
    status:      String,
    answer:      Option<String>,
    answered_at: Option<String>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn questions_path(workflow_dir: &Path) -> std::path::PathBuf {
    workflow_dir.join(QUESTIONS_FILE)
}

fn load_questions(workflow_dir: &Path) -> Vec<QuestionRecord> {
    let path = questions_path(workflow_dir);
    if !path.exists() { return vec![]; }
    let content = fs::read_to_string(&path).unwrap_or_default();
    // Last record per question_id wins
    let mut map: std::collections::HashMap<String, QuestionRecord> =
        std::collections::HashMap::new();
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        if let Ok(r) = serde_json::from_str::<QuestionRecord>(line) {
            map.insert(r.question_id.clone(), r);
        }
    }
    let mut records: Vec<QuestionRecord> = map.into_values().collect();
    records.sort_by(|a, b| b.ts.cmp(&a.ts));
    records
}

fn append_record(workflow_dir: &Path, record: &QuestionRecord) -> Result<(), ToolError> {
    let path = questions_path(workflow_dir);
    let line = serde_json::to_string(record)
        .map_err(|e| ToolError::IoError { message: format!("serialize question: {e}") })?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| ToolError::IoError { message: format!("open questions: {e}") })?;
    writeln!(file, "{}", line)
        .map_err(|e| ToolError::IoError { message: format!("write question: {e}") })?;
    Ok(())
}

// ─── sy_ask_user ─────────────────────────────────────────────────────────────

pub fn run_ask_user(
    params: AskUserParams,
    workflow_dir: &Path,
) -> Result<AskUserResult, ToolError> {
    if params.question.trim().is_empty() {
        return Err(ToolError::MissingParameter {
            missing: "question".into(),
            hint:    "Provide a non-empty question.".into(),
        });
    }

    let question_id = format!("qst_{}", Utc::now().timestamp_millis());
    let ts          = Utc::now().to_rfc3339();

    let record = QuestionRecord {
        question_id: question_id.clone(),
        ts:          ts.clone(),
        question:    params.question.clone(),
        options:     params.options.clone(),
        default:     params.default.clone(),
        status:      "pending".into(),
        answer:      None,
        answered_at: None,
    };
    append_record(workflow_dir, &record)?;

    // Build toast body
    let options_hint = params.options.as_ref()
        .map(|o| format!(" Options: {}", o.join(" / ")))
        .unwrap_or_default();
    let default_hint = params.default.as_ref()
        .map(|d| format!(" [default: {}]", d))
        .unwrap_or_default();
    let toast_body = format!("{}{}{}", params.question, options_hint, default_hint);
    let toast = win_notify::send_toast(
        "seeyue-mcp [question]",
        &toast_body,
        NotifyLevel::Warn,
    );

    // Journal
    let _ = journal::append_event(workflow_dir, JournalEvent {
        event:   "ask_user".into(),
        actor:   "tool".into(),
        payload: Some(serde_json::json!({
            "question_id": question_id,
            "question":    params.question.clone(),
        })),
        phase:    None,
        node_id:  None,
        run_id:   None,
        ts:       Utc::now().to_rfc3339(),
        trace_id: None,
    });

    // P2-N3: project into canonical interaction store (best-effort, non-blocking)
    let _ = project_ask_as_interaction(
        &question_id,
        &params.question,
        params.options.as_deref(),
        params.default.as_deref(),
        workflow_dir,
    );

    Ok(AskUserResult {
        kind:        "pending".into(),
        question_id,
        question:    params.question,
        notified:    toast.notified,
    })
}

// ─── P2-N3: Interaction Projection ───────────────────────────────────────────
//
// Projects a legacy ask_user request into the canonical interaction store
// (.ai/workflow/interactions/requests/). Additive-only — questions.jsonl unchanged.

pub fn project_ask_as_interaction(
    question_id: &str,
    question: &str,
    options: Option<&[String]>,
    default: Option<&str>,
    workflow_dir: &Path,
) -> Result<(), ToolError> {
    let requests_dir = workflow_dir.join("interactions").join("requests");
    fs::create_dir_all(&requests_dir)
        .map_err(|e| ToolError::IoError { message: format!("create interactions/requests dir: {e}") })?;

    let ts = Utc::now().to_rfc3339();
    // interaction_id must match ^ix-[0-9]{8}-[0-9]{3,}$
    let date8 = &ts[..10].replace('-', "");
    // Use last 6 digits of question_id (timestamp_ms suffix) as sequence
    let seq = question_id.trim_start_matches(|c: char| !c.is_ascii_digit())
        .chars().rev().take(6).collect::<String>()
        .chars().rev().collect::<String>();
    let seq = if seq.len() >= 3 { seq } else { format!("{:03}", 0) };
    let interaction_id = format!("ix-{}-{}", date8, seq);

    let has_options = options.map(|o| !o.is_empty()).unwrap_or(false);
    let selection_mode = if has_options { "single_select" } else { "text" };

    let ix_options: Vec<serde_json::Value> = options
        .unwrap_or(&[])
        .iter()
        .map(|o| serde_json::json!({ "id": o, "label": o, "recommended": false }))
        .collect();

    // Map `default` string to default_option_ids array (schema field)
    let default_option_ids: Option<Vec<&str>> = default.map(|d| vec![d]);

    let obj = serde_json::json!({
        "schema": 1,
        "interaction_id": interaction_id,
        "kind": "question_request",
        "status": "pending",
        "title": question,
        "message": question,
        "selection_mode": selection_mode,
        "options": ix_options,
        "default_option_ids": default_option_ids,
        "comment_mode": "disabled",
        "presentation": {
            "mode": "text_menu",
            "color_profile": "auto",
            "theme": "auto"
        },
        "originating_request_id": question_id,
        "created_at": ts,
    });

    let file_path = requests_dir.join(format!("{}.json", question_id));
    let content = serde_json::to_string_pretty(&obj)
        .map_err(|e| ToolError::IoError { message: format!("serialize ask interaction: {e}") })?;
    fs::write(&file_path, format!("{content}\n"))
        .map_err(|e| ToolError::IoError { message: format!("write ask interaction file: {e}") })?;

    Ok(())
}

// ─── sy_ask_user_status ──────────────────────────────────────────────────────

pub fn run_ask_user_status(
    params: AskUserStatusParams,
    workflow_dir: &Path,
) -> Result<AskUserStatusResult, ToolError> {
    let all = load_questions(workflow_dir);

    let filtered: Vec<QuestionEntry> = all.iter()
        .filter(|r| {
            if let Some(ref id) = params.question_id {
                &r.question_id == id
            } else {
                r.status == "pending"
            }
        })
        .map(|r| QuestionEntry {
            question_id: r.question_id.clone(),
            question:    r.question.clone(),
            options:     r.options.clone(),
            default:     r.default.clone(),
            status:      r.status.clone(),
            answer:      r.answer.clone(),
            ts:          r.ts.clone(),
            answered_at: r.answered_at.clone(),
        })
        .collect();

    let answered = filtered.iter().filter(|e| e.status == "answered").count();
    let kind = if filtered.is_empty() {
        "empty"
    } else if answered == filtered.len() {
        "answered"
    } else {
        "pending"
    };

    Ok(AskUserStatusResult {
        kind:      kind.into(),
        total:     filtered.len(),
        questions: filtered,
    })
}
