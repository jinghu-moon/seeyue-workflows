// src/platform/terminal.rs
//
// MCP 协议使用 stdout 传输 JSON-RPC 数据。
// 终端输出（颜色、diff）必须全部走 stderr，否则会污染 MCP 数据流。
//
// Windows 10 Build 14393+ 原生支持 ANSI（ENABLE_VIRTUAL_TERMINAL_PROCESSING）。
// crossterm 在 Windows 上自动调用 SetConsoleMode，旧版 Windows 回退到 Console API。
//
// Note: Term rendering API is available for future use (e.g. diff output on stderr).

use crossterm::{
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute,
            SetBackgroundColor, SetForegroundColor},
};
use std::io::{stderr, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static ANSI_ENABLED: AtomicBool = AtomicBool::new(false);

/// main() 最早处调用一次，初始化终端颜色支持
pub fn init() {
    #[cfg(windows)]
    {
        // crossterm 内部调用 SetConsoleMode + ENABLE_VIRTUAL_TERMINAL_PROCESSING
        // 失败时（旧 Windows / 重定向到文件）静默回退
        let supported = crossterm::terminal::enable_raw_mode()
            .map(|_| {
                let _ = crossterm::terminal::disable_raw_mode();
                true
            })
            .unwrap_or(false);

        // 额外检测：是否真正写到终端（而非文件重定向）
        let is_tty = crossterm::tty::IsTty::is_tty(&stderr());
        ANSI_ENABLED.store(supported && is_tty, Ordering::Relaxed);
    }

    #[cfg(not(windows))]
    {
        let is_tty = crossterm::tty::IsTty::is_tty(&stderr());
        ANSI_ENABLED.store(is_tty, Ordering::Relaxed);
    }
}

pub fn is_ansi() -> bool {
    ANSI_ENABLED.load(Ordering::Relaxed)
}

// ─── 终端渲染器 ───────────────────────────────────────────────────────────────

pub struct Term;

impl Term {
    /// Read 操作的头部信息
    pub fn print_read_header(path: &str, start: usize, end: usize, enc: &str, le: &str) {
        let mut err = stderr();
        Self::styled(&mut err, "\n  Read  ", Some(Color::Cyan), None, true);
        Self::styled(&mut err, path, None, None, true);
        Self::styled(
            &mut err,
            &format!("   Lines {start}–{end}  ({enc} · {le})\n"),
            Some(Color::DarkGrey), None, false,
        );
        writeln!(err, "  {}", dim_line(56)).ok();
    }

    /// Write 操作摘要
    pub fn print_write_summary(path: &str, action: &str, bytes: usize, lines: usize, backup: Option<&str>) {
        let mut err  = stderr();
        let action_c = if action == "created" { Color::Green } else { Color::Yellow };
        Self::styled(&mut err, "\n  Write  ", Some(Color::Cyan), None, true);
        Self::styled(&mut err, path, None, None, true);
        Self::styled(&mut err, "   ", None, None, false);
        Self::styled(&mut err, action, Some(action_c), None, false);
        Self::styled(
            &mut err,
            &format!("   {bytes} bytes · {lines} lines\n"),
            Some(Color::DarkGrey), None, false,
        );
        if let Some(bp) = backup {
            Self::styled(&mut err, &format!("  backed up → {bp}\n"), Some(Color::DarkGrey), None, false);
        }
    }

    /// Diff 渲染（Edit / Write 覆写后调用）
    ///
    /// 布局：左侧色条 | 旧行号 | 新行号 | 标记 | 内容
    /// Hunk header：Line 139 – 144  ·  2 removed  ·  2 added
    pub fn print_diff(path: &str, hunks: &[crate::diff::DiffHunk], hook: Option<&str>) {
        let mut err = stderr();

        // 文件头
        Self::styled(&mut err, "\n  ✎ ", Some(Color::Cyan), None, true);
        Self::styled(&mut err, path, None, None, true);
        writeln!(err).ok();

        for h in hunks {
            // Hunk header（本次对话确定的人类可读格式）
            let mut parts = vec![format!("Line {} – {}", h.old_start, h.old_end)];
            if h.removed > 0 { parts.push(format!("{} removed", h.removed)); }
            if h.added   > 0 { parts.push(format!("{} added",   h.added));   }
            Self::styled(
                &mut err,
                &format!("   {}  ·  {}\n", parts[0], parts[1..].join("  ·  ")),
                Some(Color::DarkGrey), None, false,
            );
            writeln!(err, "  {}", dim_line(58)).ok();

            for line in &h.lines {
                Self::print_diff_line(&mut err, line);
            }
            writeln!(err, "  {}", dim_line(58)).ok();
        }

        // 底部摘要
        if let Some(note) = hook {
            Self::styled(&mut err, &format!("  [✓ {note}]\n"), Some(Color::Green), None, false);
        }
        writeln!(err).ok();
    }

    fn print_diff_line(out: &mut impl Write, line: &crate::diff::DiffLine) {
        let old_s = line.line_old.map(|n| format!("{n:>4}")).unwrap_or_else(|| "    ".into());
        let new_s = line.line_new.map(|n| format!("{n:>4}")).unwrap_or_else(|| "    ".into());
        let content = truncate(&line.content, 70);

        if is_ansi() {
            match line.kind {
                crate::diff::DiffLineKind::Del => {
                    execute!(out,
                        SetBackgroundColor(Color::Red), Print(" "), ResetColor,
                        SetForegroundColor(Color::DarkRed),   Print(format!(" {old_s}")),
                        SetForegroundColor(Color::DarkGrey),  Print(format!(" {new_s}")),
                        SetForegroundColor(Color::Red), SetAttribute(Attribute::Bold), Print(" - "),
                        SetAttribute(Attribute::Reset),
                        SetBackgroundColor(Color::Rgb { r:60, g:0, b:0 }),
                        SetForegroundColor(Color::Red),
                        Print(format!("{content}\n")),
                        ResetColor,
                    ).ok();
                }
                crate::diff::DiffLineKind::Add => {
                    execute!(out,
                        SetBackgroundColor(Color::DarkGreen), Print(" "), ResetColor,
                        SetForegroundColor(Color::DarkGrey),  Print(format!(" {old_s}")),
                        SetForegroundColor(Color::DarkGreen), Print(format!(" {new_s}")),
                        SetForegroundColor(Color::Green), SetAttribute(Attribute::Bold), Print(" + "),
                        SetAttribute(Attribute::Reset),
                        SetBackgroundColor(Color::Rgb { r:0, g:40, b:0 }),
                        SetForegroundColor(Color::Green),
                        Print(format!("{content}\n")),
                        ResetColor,
                    ).ok();
                }
                crate::diff::DiffLineKind::Ctx => {
                    execute!(out,
                        SetForegroundColor(Color::DarkGrey),
                        Print(format!("   {old_s} {new_s}    {content}\n")),
                        ResetColor,
                    ).ok();
                }
            }
        } else {
            // 无 ANSI fallback（重定向到文件时）
            let mark = match line.kind {
                crate::diff::DiffLineKind::Del => "-",
                crate::diff::DiffLineKind::Add => "+",
                crate::diff::DiffLineKind::Ctx => " ",
            };
            writeln!(out, "  {old_s} {new_s} {mark} {content}").ok();
        }
    }

    fn styled(out: &mut impl Write, text: &str, fg: Option<Color>, bg: Option<Color>, bold: bool) {
        if !is_ansi() {
            write!(out, "{text}").ok();
            return;
        }
        if bold { execute!(out, SetAttribute(Attribute::Bold)).ok(); }
        if let Some(c) = fg { execute!(out, SetForegroundColor(c)).ok(); }
        if let Some(c) = bg { execute!(out, SetBackgroundColor(c)).ok(); }
        write!(out, "{text}").ok();
        execute!(out, ResetColor, SetAttribute(Attribute::Reset)).ok();
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn dim_line(n: usize) -> String {
    "─".repeat(n)
}
