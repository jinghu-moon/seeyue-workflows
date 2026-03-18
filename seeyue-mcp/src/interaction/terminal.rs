//! Terminal capability probing for sy-interact.
//!
//! Detects TTY status, color depth, and terminal dimensions.
//! All display output goes to stderr — stdout is reserved for structured data.
//! This module does NOT import ratatui (P1+). Uses crossterm only.

use crossterm::tty::IsTty;
use serde::{Deserialize, Serialize};

/// Color depth supported by the terminal.
/// Serializes to schema values: mono / ansi16 / ansi256 / true_color
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialOrd, Ord))]
#[serde(rename_all = "snake_case")]
pub enum ColorDepth {
    /// No color support (plain text only)
    Mono,
    /// 16 ANSI colors
    Ansi16,
    /// 256-color palette
    Ansi256,
    /// 24-bit true color
    TrueColor,
}

/// Capabilities detected from the current terminal environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    pub is_tty: bool,
    pub color_depth: ColorDepth,
    pub columns: u16,
    pub rows: u16,
    pub ansi_enabled: bool,
    pub supports_raw_mode: bool,
    pub supports_alternate_screen: bool,
    /// Recommended presentation mode given detected capabilities.
    pub preferred_mode: String,
}

/// Probe the current terminal and return detected capabilities.
pub fn probe_terminal() -> TerminalCapabilities {
    let is_tty = std::io::stderr().is_tty();

    let (columns, rows) = if is_tty {
        crossterm::terminal::size().unwrap_or((80, 24))
    } else {
        (80, 24)
    };

    let color_depth = detect_color_depth(is_tty);
    let ansi_enabled = is_tty && color_depth != ColorDepth::Mono;

    // On Windows, raw mode requires a real console handle.
    let supports_raw_mode = is_tty && probe_raw_mode();
    // Alternate screen requires raw mode support.
    let supports_alternate_screen = supports_raw_mode;

    let preferred_mode = if !is_tty {
        "plain".to_string()
    } else if supports_raw_mode {
        "tui".to_string()
    } else {
        "text".to_string()
    };

    TerminalCapabilities {
        is_tty,
        color_depth,
        columns,
        rows,
        ansi_enabled,
        supports_raw_mode,
        supports_alternate_screen,
        preferred_mode,
    }
}

/// Attempt to enter and immediately exit raw mode to check support.
fn probe_raw_mode() -> bool {
    crossterm::terminal::enable_raw_mode()
        .map(|_| {
            let _ = crossterm::terminal::disable_raw_mode();
            true
        })
        .unwrap_or(false)
}

/// Detect color depth from environment variables.
fn detect_color_depth(is_tty: bool) -> ColorDepth {
    if !is_tty {
        return ColorDepth::Mono;
    }

    // COLORTERM=truecolor or 24bit → TrueColor
    if let Ok(val) = std::env::var("COLORTERM") {
        let val = val.to_lowercase();
        if val == "truecolor" || val == "24bit" {
            return ColorDepth::TrueColor;
        }
    }

    // TERM_PROGRAM checks
    if let Ok(prog) = std::env::var("TERM_PROGRAM") {
        let prog = prog.to_lowercase();
        if prog.contains("iterm") || prog.contains("hyper") || prog == "vscode" {
            return ColorDepth::TrueColor;
        }
    }

    // TERM checks
    if let Ok(term) = std::env::var("TERM") {
        let term = term.to_lowercase();
        if term.contains("256color") {
            return ColorDepth::Ansi256;
        }
        if term == "dumb" {
            return ColorDepth::Mono;
        }
    }

    // Windows Terminal supports true color
    #[cfg(windows)]
    if std::env::var("WT_SESSION").is_ok() {
        return ColorDepth::TrueColor;
    }

    // Default: assume 16-color ANSI when TTY
    ColorDepth::Ansi16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_returns_struct() {
        let caps = probe_terminal();
        // Basic invariants
        if !caps.is_tty {
            assert!(!caps.ansi_enabled, "ansi_enabled must be false when not a TTY");
            assert!(!caps.supports_raw_mode);
            assert!(!caps.supports_alternate_screen);
            assert_eq!(caps.preferred_mode, "plain");
        }
        assert!(caps.columns > 0);
        assert!(caps.rows > 0);
    }

    #[test]
    fn test_color_depth_variants() {
        let variants = [
            (ColorDepth::Mono, "mono"),
            (ColorDepth::Ansi16, "ansi16"),
            (ColorDepth::Ansi256, "ansi256"),
            (ColorDepth::TrueColor, "true_color"),
        ];
        for (variant, expected) in &variants {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
            let decoded: ColorDepth = serde_json::from_str(&json).unwrap();
            assert_eq!(&decoded, variant);
        }
    }

    #[test]
    fn test_terminal_capabilities_serialization() {
        let caps = probe_terminal();
        let json = serde_json::to_string(&caps).unwrap();
        let decoded: TerminalCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps.is_tty, decoded.is_tty);
        assert_eq!(caps.columns, decoded.columns);
        assert_eq!(caps.preferred_mode, decoded.preferred_mode);
    }

    #[test]
    fn test_ansi_enabled_false_when_no_tty() {
        let caps = probe_terminal();
        if !caps.is_tty {
            assert!(!caps.ansi_enabled, "ansi_enabled must be false when not a TTY");
        }
    }
}
