use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

// ─── Diff 数据结构 ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffResult {
    pub hunks:          Vec<DiffHunk>,
    pub summary:        DiffSummary,
    /// 含 ANSI 颜色码的终端输出（给人看）
    pub ansi_rendered:  String,
    /// 纯文本输出（给 Agent 解析，无 ANSI）
    pub plain_rendered: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_end:   usize,
    pub new_start: usize,
    pub new_end:   usize,
    pub removed:   usize,
    pub added:     usize,
    pub lines:     Vec<DiffLine>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffLine {
    pub kind:     DiffLineKind,
    pub line_old: Option<usize>,
    pub line_new: Option<usize>,
    pub content:  String,    // 不含行号前缀，不含 +/-
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineKind { Del, Add, Ctx }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffSummary {
    pub total_removed: usize,
    pub total_added:   usize,
}

// ─── ANSI 颜色 ────────────────────────────────────────────────────────────────

const RED:         &str = "\x1b[31m";
const GREEN:       &str = "\x1b[32m";
const GRAY:        &str = "\x1b[90m";
const BOLD:        &str = "\x1b[1m";
const DIM:         &str = "\x1b[2m";
const RESET:       &str = "\x1b[0m";
const BG_RED:      &str = "\x1b[48;5;52m";   // 深红背景
const BG_GREEN:    &str = "\x1b[48;5;22m";   // 深绿背景
const BAR_RED:     &str = "\x1b[41m \x1b[0m"; // 左侧红竖条
const BAR_GREEN:   &str = "\x1b[42m \x1b[0m"; // 左侧绿竖条

// ─── 主入口 ───────────────────────────────────────────────────────────────────

pub fn compute_diff(
    file_path: &str,
    old_text:  &str,
    new_text:  &str,
    hook_note: Option<&str>,  // e.g. "✓ rustfmt"
) -> DiffResult {

    let td = TextDiff::from_lines(old_text, new_text);

    let mut hunks       = Vec::new();
    let mut total_rem   = 0usize;
    let mut total_add   = 0usize;

    for group in td.grouped_ops(2) {      // 2 行上下文
        let mut lines     = Vec::new();
        let mut old_line  = 0usize;
        let mut new_line  = 0usize;
        let mut first     = true;
        let mut hunk_rem  = 0usize;
        let mut hunk_add  = 0usize;
        let mut old_start = 0usize;
        let mut new_start = 0usize;

        for op in &group {
            for change in td.iter_inline_changes(op) {
                let tag = change.tag();

                if first {
                    old_start = change.old_index().unwrap_or(0) + 1;
                    new_start = change.new_index().unwrap_or(0) + 1;
                    first     = false;
                }

                let content = change
                    .iter_strings_lossy()
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>()
                    .join("");
                let content = content.trim_end_matches('\n').to_string();

                match tag {
                    ChangeTag::Delete => {
                        old_line = change.old_index().unwrap_or(old_line) + 1;
                        lines.push(DiffLine {
                            kind:     DiffLineKind::Del,
                            line_old: Some(old_line),
                            line_new: None,
                            content,
                        });
                        hunk_rem += 1;
                    }
                    ChangeTag::Insert => {
                        new_line = change.new_index().unwrap_or(new_line) + 1;
                        lines.push(DiffLine {
                            kind:     DiffLineKind::Add,
                            line_old: None,
                            line_new: Some(new_line),
                            content,
                        });
                        hunk_add += 1;
                    }
                    ChangeTag::Equal => {
                        old_line = change.old_index().unwrap_or(old_line) + 1;
                        new_line = change.new_index().unwrap_or(new_line) + 1;
                        lines.push(DiffLine {
                            kind:     DiffLineKind::Ctx,
                            line_old: Some(old_line),
                            line_new: Some(new_line),
                            content,
                        });
                    }
                }
            }
        }

        let old_end = old_start + lines.iter().filter(|l| l.line_old.is_some()).count();
        let new_end = new_start + lines.iter().filter(|l| l.line_new.is_some()).count();

        total_rem += hunk_rem;
        total_add += hunk_add;

        hunks.push(DiffHunk {
            old_start,
            old_end,
            new_start,
            new_end,
            removed: hunk_rem,
            added:   hunk_add,
            lines,
        });
    }

    let summary = DiffSummary {
        total_removed: total_rem,
        total_added:   total_add,
    };

    let ansi_rendered  = render_ansi(file_path, &hunks, &summary, hook_note);
    let plain_rendered = render_plain(file_path, &hunks, &summary, hook_note);

    DiffResult { hunks, summary, ansi_rendered, plain_rendered }
}

// ─── 终端 ANSI 渲染（人看）────────────────────────────────────────────────────

fn render_ansi(
    file_path: &str,
    hunks:     &[DiffHunk],
    summary:   &DiffSummary,
    hook_note: Option<&str>,
) -> String {
    let mut out = String::new();

    out.push('\n');
    // 文件头
    out.push_str(&format!(" {BOLD}{}\x1b[36m ✎ {file_path}{RESET}\n",
        "", // 前置空格对齐
    ));
    // 覆盖掉上面的格式，实际输出：
    out.clear();
    out.push('\n');
    out.push_str(&format!(" {BOLD}\x1b[36m✎ {file_path}{RESET}\n"));

    for hunk in hunks {
        // Hunk header：人类可读格式
        out.push_str(&render_hunk_header_ansi(hunk));
        out.push('\n');

        // 分隔线
        let sep_width = 56usize;
        out.push_str(&format!("  {GRAY}{}{RESET}\n", "─".repeat(sep_width)));

        // 每一行
        for line in &hunk.lines {
            out.push_str(&render_line_ansi(line));
            out.push('\n');
        }

        // 底部分隔线
        out.push_str(&format!("  {GRAY}{}{RESET}\n", "─".repeat(sep_width)));
    }

    // 底部摘要
    let mut footer_parts = Vec::new();
    if summary.total_removed > 0 || summary.total_added > 0 {
        if summary.total_removed > 0 {
            footer_parts.push(format!("{RED}-{}{RESET}", summary.total_removed));
        }
        if summary.total_added > 0 {
            footer_parts.push(format!("{GREEN}+{}{RESET}", summary.total_added));
        }
    }
    if let Some(note) = hook_note {
        footer_parts.push(format!("{GREEN}[✓ {note}]{RESET}"));
    }
    if !footer_parts.is_empty() {
        out.push_str(&format!("   {}\n", footer_parts.join("  ")));
    }
    out.push('\n');

    out
}

fn render_hunk_header_ansi(hunk: &DiffHunk) -> String {
    // "  Line 139 – 144  ·  2 removed  ·  2 added"
    let range = format!("Line {} – {}", hunk.old_start, hunk.old_end);
    let mut parts = vec![format!("{GRAY}{range}{RESET}")];
    if hunk.removed > 0 {
        parts.push(format!("{RED}{} removed{RESET}", hunk.removed));
    }
    if hunk.added > 0 {
        parts.push(format!("{GREEN}{} added{RESET}", hunk.added));
    }
    let mid = format!("  {GRAY}·{RESET}  ");
    format!("  {}", parts.join(&mid))
}

fn render_line_ansi(line: &DiffLine) -> String {
    let num_width = 4usize;

    let old_num = match line.line_old {
        Some(n) => format!("{:>width$}", n, width = num_width),
        None    => " ".repeat(num_width),
    };
    let new_num = match line.line_new {
        Some(n) => format!("{:>width$}", n, width = num_width),
        None    => " ".repeat(num_width),
    };

    match line.kind {
        DiffLineKind::Del => format!(
            "{BAR_RED} {RED}{old_num}{RESET} {GRAY}{new_num}{RESET} {BOLD}{RED} - {RESET}{BG_RED}{RED}{}{RESET}",
            truncate(&line.content, 72)
        ),
        DiffLineKind::Add => format!(
            "{BAR_GREEN} {GRAY}{old_num}{RESET} {GREEN}{new_num}{RESET} {BOLD}{GREEN} + {RESET}{BG_GREEN}{GREEN}{}{RESET}",
            truncate(&line.content, 72)
        ),
        DiffLineKind::Ctx => format!(
            "   {DIM}{old_num}{RESET} {DIM}{new_num}{RESET} {DIM}   {}{RESET}",
            truncate(&line.content, 72)
        ),
    }
}

// ─── 纯文本渲染（Agent 解析用）────────────────────────────────────────────────

fn render_plain(
    file_path: &str,
    hunks:     &[DiffHunk],
    summary:   &DiffSummary,
    hook_note: Option<&str>,
) -> String {
    let mut out = String::new();

    out.push_str(&format!("✎ {file_path}\n"));

    for hunk in hunks {
        let range = format!("Line {} – {}", hunk.old_start, hunk.old_end);
        let mut parts = vec![range];
        if hunk.removed > 0 { parts.push(format!("{} removed", hunk.removed)); }
        if hunk.added   > 0 { parts.push(format!("{} added",   hunk.added));   }
        out.push_str(&format!("  {}\n", parts.join("  ·  ")));
        out.push_str(&format!("  {}\n", "─".repeat(56)));

        for line in &hunk.lines {
            let old_s = line.line_old.map(|n| n.to_string()).unwrap_or_else(|| "    ".into());
            let new_s = line.line_new.map(|n| n.to_string()).unwrap_or_else(|| "    ".into());
            let mark = match line.kind {
                DiffLineKind::Del => "-",
                DiffLineKind::Add => "+",
                DiffLineKind::Ctx => " ",
            };
            out.push_str(&format!(
                "  {:>4} {:>4} {} {}\n",
                old_s, new_s, mark, line.content
            ));
        }
        out.push_str(&format!("  {}\n", "─".repeat(56)));
    }

    let mut footer = Vec::new();
    if summary.total_removed > 0 { footer.push(format!("-{}", summary.total_removed)); }
    if summary.total_added   > 0 { footer.push(format!("+{}", summary.total_added));   }
    if let Some(note) = hook_note { footer.push(format!("[✓ {note}]")); }
    if !footer.is_empty() {
        out.push_str(&format!("  {}\n", footer.join("  ")));
    }

    out
}

fn truncate(s: &str, max: usize) -> String {
    let visible: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        format!("{visible}…")
    } else {
        visible
    }
}
