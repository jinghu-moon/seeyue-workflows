use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::storage::backup::{BackupManager, BackupTrigger};
use crate::storage::cache::ReadCache;
use crate::storage::checkpoint::CheckpointStore;
use crate::render::diff::{compute_diff, DiffResult};
use crate::encoding::{
    find_unicode_confusion, safe_read, safe_write, sha256_hex,
    try_tab_normalized_match,
};
use crate::error::{EditPreview, MatchLocation, ToolError};
use crate::tools::read::resolve_path;

// ─── Edit 参数与响应 ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EditParams {
    pub file_path:    String,
    pub old_string:   String,
    pub new_string:   String,
    /// 替换所有出现（批量重命名）
    #[serde(default, deserialize_with = "de_bool")]
    pub replace_all:  bool,
    /// 跳过 modified 校验（外部 formatter 持续修改时使用）
    #[serde(default, deserialize_with = "de_bool")]
    pub force:        bool,
}

#[derive(Debug, Serialize)]
pub struct EditResult {
    #[serde(rename = "type")]
    pub kind:         String,           // "success"
    pub file_path:    String,
    pub replacements: usize,
    pub match_kind:   String,           // "exact" | "tab_normalized"
    pub backup_path:  Option<String>,
    pub backup_note:  Option<String>,
    pub diff:         DiffResult,
}

// ─── MultiEdit 参数与响应 ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MultiEditParams {
    pub file_path: String,
    pub edits:     Vec<SingleEdit>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SingleEdit {
    pub old_string:             String,
    pub new_string:             String,
    #[serde(default, deserialize_with = "de_bool")]
    pub replace_all:            bool,
    pub expected_replacements:  Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct MultiEditResult {
    #[serde(rename = "type")]
    pub kind:          String,
    pub file_path:     String,
    pub edits_applied: usize,
    pub backup_path:   Option<String>,
    pub diff:          DiffResult,
}

// ─── Edit 内存替换（preview_edit 复用）──────────────────────────────────────

#[derive(Debug)]
pub struct EditApplyResult {
    pub new_content: String,
    pub replacements: usize,
    pub match_kind: String,
}

pub fn apply_edit_in_memory(
    old_string:  &str,
    new_string:  &str,
    replace_all: bool,
    content:     &str,
) -> Result<EditApplyResult, ToolError> {
    let (matched_old, match_kind, count) =
        match_string(old_string, content, replace_all)?;

    let new_content = if replace_all || count > 1 {
        content.replace(&matched_old[..], new_string)
    } else {
        content.replacen(&matched_old[..], new_string, 1)
    };

    let replacements = if replace_all { count } else { 1 };

    Ok(EditApplyResult {
        new_content,
        replacements,
        match_kind: match_kind.to_string(),
    })
}

// ─── Edit 主逻辑 ──────────────────────────────────────────────────────────────

pub fn run_edit(
    params:     EditParams,
    cache:      &ReadCache,
    checkpoint: &CheckpointStore,
    backup_mgr: &BackupManager,
    workspace:  &Path,
    call_id:    &str,
) -> Result<EditResult, ToolError> {

    let path = resolve_path(workspace, &params.file_path)?;

    // ── Step 0a: 参数校验 ─────────────────────────────────────────────────
    if params.old_string == params.new_string {
        return Err(ToolError::NoChanges {
            hint: "old_string and new_string are identical (byte-level). No edit performed.".into(),
            unicode_note: Some(
                "If you intended to change quote styles (e.g. ' → \u{2019}), verify the Unicode code points.".into()
            ),
        });
    }

    // ── Step 0b: 文件存在 ─────────────────────────────────────────────────
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.file_path.clone(),
            hint: "File does not exist. Use Write to create it.".into(),
        });
    }

    // ── Step 1: modified 校验（force=false 时）────────────────────────────
    if !params.force {
        let entry = cache.get(&path).ok_or_else(|| ToolError::FileNotRead {
            file_path: params.file_path.clone(),
            hint: "Call Read before Edit to ensure old_string matches exactly.".into(),
        })?;

        let current_raw = std::fs::read(&path).map_err(|e| ToolError::IoError {
            message: e.to_string(),
        })?;
        let current_hash = sha256_hex(&current_raw);

        if current_hash != entry.raw_hash && current_hash != entry.norm_hash {
            return Err(ToolError::FileModified {
                file_path: params.file_path.clone(),
                read_at:   entry.read_at.to_rfc3339(),
                hint: "File was modified externally after Read. Call Read again.".into(),
                tip: "If a formatter (Prettier/rustfmt) is running, use force=true to skip this check.".into(),
            });
        }
    }

    // ── Step 2: 安全读取（编码层）────────────────────────────────────────
    let file_data = safe_read(&path)?;
    let content   = &file_data.content;

    // ── Step 3: 字符串匹配 ────────────────────────────────────────────────
    let applied = apply_edit_in_memory(
        &params.old_string,
        &params.new_string,
        params.replace_all,
        content,
    )?;

    // ── Step 4: 写前备份 ─────────────────────────────────────────────────
    let backup_trigger = if cache.edit_count(&path) == 0 {
        BackupTrigger::FirstEdit
    } else {
        BackupTrigger::AllWrites  // 只有 AllWrites 模式才会再次备份
    };

    let backup_record = backup_mgr.backup(
        &path,
        "Edit",
        backup_trigger,
        &file_data.encoding.name,
        &format!("{:?}", file_data.encoding.line_ending),
    )?;

    // ── Step 5: Checkpoint 快照 ───────────────────────────────────────────
    checkpoint.capture(&path, call_id, "Edit")?;

    // ── Step 6: 执行替换 ──────────────────────────────────────────────────
    let new_content = applied.new_content;
    let replacements = applied.replacements;

    // ── Step 7: 编码安全写入 ──────────────────────────────────────────────
    safe_write(
        &path,
        &new_content,
        &file_data.encoding,
        false,
        !file_data.encoding.has_non_ascii,
    )?;

    // ── Step 8: 更新缓存 hash ─────────────────────────────────────────────
    let new_raw  = std::fs::read(&path).unwrap_or_default();
    let new_hash = sha256_hex(&new_raw);
    let new_norm = sha256_hex(new_content.replace("\r\n", "\n").as_bytes());
    cache.update_after_edit(&path, new_hash, new_norm);

    // ── Step 9: diff 渲染 ─────────────────────────────────────────────────
    let diff = compute_diff(&params.file_path, content, &new_content, None);
    eprint!("{}", diff.ansi_rendered);

    // ── 备份提示 ──────────────────────────────────────────────────────────
    let (backup_path, backup_note) = format_backup_info(backup_record.as_ref());

    Ok(EditResult {
        kind:         "success".into(),
        file_path:    params.file_path,
        replacements,
        match_kind:   applied.match_kind,
        backup_path,
        backup_note,
        diff,
    })
}

// ─── MultiEdit 主逻辑 ─────────────────────────────────────────────────────────

pub fn run_multi_edit(
    params:     MultiEditParams,
    cache:      &ReadCache,
    checkpoint: &CheckpointStore,
    backup_mgr: &BackupManager,
    workspace:  &Path,
    call_id:    &str,
) -> Result<MultiEditResult, ToolError> {

    let path = resolve_path(workspace, &params.file_path)?;

    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.file_path.clone(),
            hint: "File does not exist.".into(),
        });
    }

    // modified 校验
    if let Some(entry) = cache.get(&path) {
        let current_hash = sha256_hex(&std::fs::read(&path).unwrap_or_default());
        if current_hash != entry.raw_hash && current_hash != entry.norm_hash {
            return Err(ToolError::FileModified {
                file_path: params.file_path.clone(),
                read_at:   entry.read_at.to_rfc3339(),
                hint: "File was modified externally. Call Read again.".into(),
                tip: "Use force=true on individual Edits if a formatter is running.".into(),
            });
        }
    } else {
        return Err(ToolError::FileNotRead {
            file_path: params.file_path.clone(),
            hint: "Call Read before MultiEdit.".into(),
        });
    }

    let file_data = safe_read(&path)?;
    let original  = file_data.content.clone();

    // ── Phase 1：全量预校验（任一失败 → 整体失败，文件不动）─────────────
    let mut working = original.clone();
    let mut validated_edits: Vec<(String, String, bool)> = Vec::new();

    for (idx, edit) in params.edits.iter().enumerate() {
        let (matched, _, count) =
            match_string_for_multi(&edit.old_string, &working, edit.replace_all, edit.expected_replacements)
            .map_err(|cause| ToolError::EditFailed {
                edit_index:   idx,
                edit_preview: EditPreview {
                    old_string: edit.old_string.chars().take(80).collect(),
                    new_string: edit.new_string.chars().take(80).collect(),
                },
                cause: Box::new(cause),
                file_state: "unchanged".into(),
                hint: format!("Fix edit[{idx}] and retry the entire MultiEdit."),
            })?;

        // 在 working copy 上串行应用，后续 edit 看到前面 edit 的结果
        working = if edit.replace_all || count > 1 {
            working.replace(&matched[..], &edit.new_string)
        } else {
            working.replacen(&matched[..], &edit.new_string, 1)
        };

        validated_edits.push((matched, edit.new_string.clone(), edit.replace_all));
    }

    // ── Phase 2：全部通过，执行写入 ──────────────────────────────────────
    let backup_record = backup_mgr.backup(
        &path, "MultiEdit",
        BackupTrigger::FirstEdit,
        &file_data.encoding.name,
        &format!("{:?}", file_data.encoding.line_ending),
    )?;

    checkpoint.capture(&path, call_id, "MultiEdit")?;

    safe_write(&path, &working, &file_data.encoding, false, !file_data.encoding.has_non_ascii)?;

    let new_raw  = std::fs::read(&path).unwrap_or_default();
    let new_hash = sha256_hex(&new_raw);
    let new_norm = sha256_hex(working.replace("\r\n", "\n").as_bytes());
    cache.update_after_edit(&path, new_hash, new_norm);

    let diff = compute_diff(&params.file_path, &original, &working, None);
    eprint!("{}", diff.ansi_rendered);

    let (backup_path, _) = format_backup_info(backup_record.as_ref());

    Ok(MultiEditResult {
        kind:          "success".into(),
        file_path:     params.file_path,
        edits_applied: validated_edits.len(),
        backup_path,
        diff,
    })
}

// ─── 字符串匹配（三级 fallback）──────────────────────────────────────────────

/// 返回 (实际匹配的 old_string, 匹配类型, 匹配次数)
fn match_string(
    old_string:  &str,
    content:     &str,
    replace_all: bool,
) -> Result<(String, &'static str, usize), ToolError> {

    // Level 0: 精确匹配
    let count = count_occurrences(old_string, content);

    if count == 1 || (replace_all && count >= 1) {
        return Ok((old_string.to_string(), "exact", count));
    }

    if count >= 2 && !replace_all {
        let locations = find_match_locations(old_string, content);
        return Err(ToolError::MultipleMatches {
            count,
            locations,
            hint: format!(
                "Add surrounding context to old_string to uniquify, \
                 or set replace_all=true to replace all {count}."
            ),
        });
    }

    // Level A: Tab/Space 规范化重试
    if let Some(normalized) = try_tab_normalized_match(old_string, content) {
        let norm_count = count_occurrences(&normalized, content);
        if norm_count == 1 || (replace_all && norm_count >= 1) {
            // 告知 Agent 使用了 tab 规范化
            eprintln!(
                "\x1b[33m[Tab normalization]\x1b[0m \
                 old_string was matched after tab/space normalization. \
                 Use the exact indentation from Read output next time."
            );
            return Ok((normalized, "tab_normalized", norm_count));
        }
    }

    // Level B: Unicode 混淆检测
    let unicode_hints = find_unicode_confusion(old_string, content);

    // Level C: STRING_NOT_FOUND（含所有诊断信息 + 最近 3 行）
    let first_line = old_string.lines().next().unwrap_or(old_string);
    let similar = find_similar_lines(first_line, content, 3);
    let similar_hint = if similar.is_empty() {
        String::new()
    } else {
        let lines = similar.join("\n  ");
        format!("\nNearest lines in file:\n  {lines}")
    };

    Err(ToolError::StringNotFound {
        file_path:          "[file]".into(), // 调用方填充
        old_string_preview: old_string.chars().take(80).collect(),
        suggestions:        unicode_hints,
        hint: format!(
            "Read the file again and copy old_string verbatim from the output. \
             Pay attention to tab characters (shown as \\t in Read output).{similar_hint}"
        ),
    })
}

/// 按首行相似度在文件中查找最接近的 n 行。
/// 相似度 = 公共前缀字节数 * 2 + 公共字符数（不区分大小写）。
fn find_similar_lines(first_line: &str, content: &str, n: usize) -> Vec<String> {
    let needle_lower = first_line.trim().to_lowercase();
    let needle_bytes = first_line.trim().as_bytes();

    let mut scored: Vec<(usize, usize, &str)> = content
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            // 公共前缀字节数
            let prefix = needle_bytes
                .iter()
                .zip(trimmed.as_bytes())
                .take_while(|(a, b)| a == b)
                .count();
            // 公共字符占比（不区分大小写）
            let line_lower = trimmed.to_lowercase();
            let common_chars = needle_lower
                .chars()
                .filter(|c| line_lower.contains(*c))
                .count();
            let score = prefix * 2 + common_chars;
            if score == 0 {
                None
            } else {
                Some((score, idx + 1, line))
            }
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
        .into_iter()
        .take(n)
        .map(|(_, lineno, text)| format!("L{lineno}: {}", text.trim_end()))
        .collect()
}

fn match_string_for_multi(
    old_string:           &str,
    content:              &str,
    replace_all:          bool,
    expected:             Option<usize>,
) -> Result<(String, &'static str, usize), ToolError> {
    let count = count_occurrences(old_string, content);

    if let Some(exp) = expected {
        if count == exp {
            return Ok((old_string.to_string(), "exact", count));
        } else {
            return Err(ToolError::MultipleMatches {
                count,
                locations: find_match_locations(old_string, content),
                hint: format!(
                    "expected_replacements={exp} but found {count} matches."
                ),
            });
        }
    }

    match_string(old_string, content, replace_all)
}

fn count_occurrences(needle: &str, haystack: &str) -> usize {
    if needle.is_empty() { return 0; }
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        count += 1;
        start += pos + needle.len();
    }
    count
}

fn find_match_locations(needle: &str, haystack: &str) -> Vec<MatchLocation> {
    let mut locations = Vec::new();
    let mut line_no   = 1usize;
    let mut char_pos  = 0usize;

    for (byte_idx, ch) in haystack.char_indices() {
        if ch == '\n' { line_no += 1; char_pos = byte_idx + 1; }

        if haystack[byte_idx..].starts_with(needle) {
            // 找到这一行的内容
            let line_end = haystack[byte_idx..]
                .find('\n')
                .map(|p| byte_idx + p)
                .unwrap_or(haystack.len());
            let context = haystack[char_pos..line_end]
                .chars().take(72).collect();

            locations.push(MatchLocation { line: line_no, context });
            if locations.len() >= 10 { break; }  // 最多报告 10 个
        }
    }

    locations
}

// ─── 格式化备份信息 ───────────────────────────────────────────────────────────

fn format_backup_info(
    record: Option<&crate::storage::backup::BackupRecord>,
) -> (Option<String>, Option<String>) {
    match record {
        Some(r) => (Some(r.backup_path.display().to_string()), None),
        None    => (None, Some("Backup already exists from this session.".into())),
    }
}

// ─── 辅助：宽松 bool 反序列化（接受 "true"/"false" 字符串）──────────────────

fn de_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrStr { B(bool), S(String) }
    match BoolOrStr::deserialize(d)? {
        BoolOrStr::B(b) => Ok(b),
        BoolOrStr::S(s) => match s.to_lowercase().as_str() {
            "true"  | "1" | "yes" => Ok(true),
            "false" | "0" | "no"  => Ok(false),
            other => Err(serde::de::Error::custom(
                format!("invalid bool string: {other}")
            )),
        },
    }
}
