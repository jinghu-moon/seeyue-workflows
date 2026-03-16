use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::backup::{BackupManager, BackupTrigger};
use crate::cache::ReadCache;
use crate::checkpoint::CheckpointStore;
use crate::diff::{compute_diff, DiffResult};
use crate::encoding::{safe_read, safe_write, sha256_hex, EncodingInfo, LineEnding};
use crate::error::ToolError;
use crate::tools::read::resolve_path;

// ─── Write 参数与响应 ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WriteParams {
    pub file_path: String,
    pub content:   String,
}

#[derive(Debug, Serialize)]
pub struct WriteResult {
    #[serde(rename = "type")]
    pub kind:        String,       // "success"
    pub file_path:   String,
    pub action:      String,       // "created" | "overwritten"
    pub bytes:       usize,
    pub lines:       usize,
    pub backup_path: Option<String>,
    pub diff:        Option<DiffResult>,
}

// ─── Write 主逻辑 ─────────────────────────────────────────────────────────────

pub fn run_write(
    params:     WriteParams,
    cache:      &ReadCache,
    checkpoint: &CheckpointStore,
    backup_mgr: &BackupManager,
    workspace:  &Path,
    call_id:    &str,
) -> Result<WriteResult, ToolError> {

    let path    = resolve_path(workspace, &params.file_path)?;
    let is_new  = !path.exists();

    // ── 已有文件必须先 Read（防止盲目覆写）──────────────────────────────
    if !is_new && !cache.has_been_read(&path) {
        return Err(ToolError::FileNotRead {
            file_path: params.file_path.clone(),
            hint: "File exists but has not been read in this session. \
                   Call Read first to avoid accidental overwrites.".into(),
        });
    }

    // ── 读取原始内容（用于 diff 和编码信息）─────────────────────────────
    let (old_content, enc_info) = if is_new {
        (String::new(), EncodingInfo {
            name:          "UTF-8".into(),
            confidence:    1.0,
            bom:           None,
            has_non_ascii: false,
            line_ending:   LineEnding::Lf,
        })
    } else {
        let data = safe_read(&path)?;
        (data.content, data.encoding)
    };

    // ── 备份（仅覆写已有文件）────────────────────────────────────────────
    let backup_record = if !is_new {
        backup_mgr.backup(
            &path, "Write",
            BackupTrigger::FirstEdit,
            &enc_info.name,
            &format!("{:?}", enc_info.line_ending),
        )?
    } else {
        None
    };

    // ── Checkpoint ────────────────────────────────────────────────────────
    checkpoint.capture(&path, call_id, "Write")?;

    // ── 编码安全写入 ──────────────────────────────────────────────────────
    let orig_was_ascii = !enc_info.has_non_ascii;
    safe_write(&path, &params.content, &enc_info, is_new, orig_was_ascii)?;

    // ── 更新缓存 ──────────────────────────────────────────────────────────
    let new_raw  = std::fs::read(&path).unwrap_or_default();
    let new_hash = sha256_hex(&new_raw);
    let new_norm = sha256_hex(params.content.replace("\r\n", "\n").as_bytes());
    cache.insert(path.clone(), new_hash, new_norm, &enc_info);

    // ── Diff 渲染（覆写时）────────────────────────────────────────────────
    let diff = if !is_new && old_content != params.content {
        let d = compute_diff(&params.file_path, &old_content, &params.content, None);
        eprint!("{}", d.ansi_rendered);
        Some(d)
    } else {
        None
    };

    // ── 终端摘要 ──────────────────────────────────────────────────────────
    let action = if is_new { "created" } else { "overwritten" };
    let bytes  = params.content.len();
    let lines  = params.content.lines().count();
    let bp     = backup_record.as_ref().map(|r| r.backup_path.display().to_string());

    eprintln!(
        " \x1b[1m\x1b[36m Write\x1b[0m  \x1b[1m{}\x1b[0m   \x1b[90m{action}   {bytes} bytes · {lines} lines\x1b[0m{}",
        params.file_path,
        bp.as_deref().map(|p| format!("\n   \x1b[90mbacked up → {p}\x1b[0m")).unwrap_or_default(),
    );

    Ok(WriteResult {
        kind:        "success".into(),
        file_path:   params.file_path,
        action:      action.into(),
        bytes,
        lines,
        backup_path: bp,
        diff,
    })
}
