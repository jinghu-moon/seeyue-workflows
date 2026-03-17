use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::storage::cache::ReadCache;
use crate::encoding::safe_read;
use crate::error::ToolError;

const MAX_LINES: usize = 2000;

// ─── 参数与响应 ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReadParams {
    pub file_path:   String,
    pub start_line:  Option<usize>,   // 1-based
    pub end_line:    Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ReadResult {
    #[serde(rename = "type")]
    pub kind:        String,          // "success"
    pub file_path:   String,
    pub total_lines: usize,
    pub start_line:  usize,
    pub end_line:    usize,
    pub content:     String,          // 带行号前缀
    pub encoding:    String,
    pub line_ending: String,
    pub truncated:   bool,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_read(
    params: ReadParams,
    cache:  &ReadCache,
    workspace: &Path,
) -> Result<ReadResult, ToolError> {

    let path = resolve_path(workspace, &params.file_path)?;

    // ── 文件存在检查 ───────────────────────────────────────────────────────
    if !path.exists() {
        return Err(ToolError::FileNotFound {
            file_path: params.file_path.clone(),
            hint: "File does not exist. Use Write to create it.".into(),
        });
    }

    // ── 二进制检查（前 512 字节）──────────────────────────────────────────
    {
        let header = read_bytes_head(&path, 512)?;
        if is_binary(&header) {
            return Err(ToolError::BinaryFile {
                file_path: params.file_path.clone(),
                hint: "Binary file cannot be read as text. Use Bash to inspect.".into(),
            });
        }
    }

    // ── 安全读取（编码检测 + 解码）───────────────────────────────────────
    let result = safe_read(&path)?;

    // ── 写入缓存（供 Edit / Write 校验）──────────────────────────────────
    cache.insert(
        path.clone(),
        result.raw_hash.clone(),
        result.norm_hash.clone(),
        &result.encoding,
    );

    // ── 行切分 ────────────────────────────────────────────────────────────
    let all_lines: Vec<&str> = result.content.lines().collect();
    let total_lines           = all_lines.len();

    // 行范围校验
    let start = params.start_line.unwrap_or(1).max(1);
    let end   = params.end_line.unwrap_or(total_lines).min(total_lines);

    if start > end && total_lines > 0 {
        return Err(ToolError::InvalidLineRange {
            start_line:  start,
            end_line:    end,
            total_lines,
            hint: "start_line must be <= end_line.".into(),
        });
    }

    // 截断保护
    let actual_end = if end - start + 1 > MAX_LINES {
        start + MAX_LINES - 1
    } else {
        end
    };
    let truncated = actual_end < end;

    // ── 构建带行号的输出（tab 保持原样，不转空格）────────────────────────
    let num_width = total_lines.to_string().len().max(3);
    let mut content_lines = Vec::new();

    for (i, line) in all_lines.iter().enumerate() {
        let line_no = i + 1;
        if line_no < start  { continue; }
        if line_no > actual_end { break; }

        // 行号 + tab（原样保留，不转空格）
        content_lines.push(format!("{:>width$}\t{}", line_no, line, width = num_width));
    }

    let mut content = content_lines.join("\n");

    if truncated {
        content.push_str(&format!(
            "\n\n[截断：文件共 {total_lines} 行，已显示第 {start}–{actual_end} 行。\n \
             使用 start_line/end_line 参数读取后续内容。]"
        ));
    }

    // ── 终端打印 ──────────────────────────────────────────────────────────
    print_read_header(&params.file_path, start, actual_end, &result.encoding.name, &result.encoding.line_ending);
    eprintln!("{content}");
    eprintln!("\x1b[90m{}\x1b[0m", "─".repeat(56));

    Ok(ReadResult {
        kind:        "success".into(),
        file_path:   params.file_path,
        total_lines,
        start_line:  start,
        end_line:    actual_end,
        content,
        encoding:    result.encoding.name.clone(),
        line_ending: format!("{:?}", result.encoding.line_ending),
        truncated,
    })
}

// ─── 辅助 ────────────────────────────────────────────────────────────────────

/// Delegate to [`crate::platform::path::resolve`] which uses pure string
/// comparison instead of `fs::canonicalize`.  The old implementation relied on
/// `canonicalize` for the escape check, which fails for **new files** on Windows
/// because `canonicalize` adds a `\\?\` UNC prefix only when the path exists,
/// causing a prefix mismatch against the (existing) workspace root.
pub fn resolve_path(workspace: &Path, requested: &str) -> Result<PathBuf, ToolError> {
    crate::platform::path::resolve(workspace, requested)
}

fn read_bytes_head(path: &Path, n: usize) -> Result<Vec<u8>, ToolError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| ToolError::IoError {
        message: e.to_string(),
    })?;
    let mut buf = vec![0u8; n];
    let read = f.read(&mut buf).map_err(|e| ToolError::IoError {
        message: e.to_string(),
    })?;
    buf.truncate(read);
    Ok(buf)
}

/// Binary detection heuristic adapted from `content_inspector` (sharkdp)
/// and `git diff`.
///
/// Algorithm:
///   1. BOM present → text (short-circuit; UTF-32 checked before UTF-16
///      because their BOMs share a prefix).
///   2. NULL byte in buffer → binary (same rule as `git diff`).
///   3. Otherwise → text.
///
/// Previous implementation counted bytes in 0x7F–0xBF as "non-UTF-8" and
/// rejected files where they exceeded 33%. This caused false positives for
/// CJK text, whose UTF-8 continuation bytes (0x80–0xBF) make up 2/3 of
/// all bytes.
fn is_binary(bytes: &[u8]) -> bool {
    // ── Step 1: BOM → definitely text ────────────────────────────────
    // Order matters: UTF-32 BOMs overlap with UTF-16 BOMs.
    const TEXT_BOMS: &[&[u8]] = &[
        &[0xEF, 0xBB, 0xBF],       // UTF-8 BOM
        &[0x00, 0x00, 0xFE, 0xFF],  // UTF-32BE BOM
        &[0xFF, 0xFE, 0x00, 0x00],  // UTF-32LE BOM
        &[0xFE, 0xFF],              // UTF-16BE BOM
        &[0xFF, 0xFE],              // UTF-16LE BOM
    ];
    if TEXT_BOMS.iter().any(|bom| bytes.starts_with(bom)) {
        return false;
    }

    // ── Step 2: NULL byte → binary ───────────────────────────────────
    bytes.contains(&0)
}

fn print_read_header(
    path:        &str,
    start:       usize,
    end:         usize,
    encoding:    &str,
    line_ending: &crate::encoding::LineEnding,
) {
    let le = match line_ending {
        crate::encoding::LineEnding::Lf    => "LF",
        crate::encoding::LineEnding::Crlf  => "CRLF",
        crate::encoding::LineEnding::Mixed => "mixed",
    };
    eprintln!(
        "\n \x1b[1m\x1b[36m Read\x1b[0m  \x1b[1m{path}\x1b[0m   \
         \x1b[90mLines {start} – {end}  ({encoding} · {le})\x1b[0m"
    );
    eprintln!("\x1b[90m{}\x1b[0m", "─".repeat(56));
}
