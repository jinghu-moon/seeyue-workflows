//! Ratatui-based TUI renderer for sy-interact.
//!
//! P1 implementation: full keyboard-first menu with:
//! - Single/multi-select option list with focus management
//! - Optional/required comment input panel
//! - Details panel toggle (Tab)
//! - Status/help bar
//! - Theme token application
//!
//! Entry point: `render_tui()` — consumes request, returns TerminalResponse.

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal, TerminalOptions, Viewport,
};
use std::io::Stderr;
use std::time::Duration;

use crate::interaction::{
    renderer::TerminalResponse,
    schema::{
        CommentMode, InteractionRequest, ResponseStatus, SelectionMode,
    },
    state::{FocusPanel, TuiState},
    theme::Theme,
};

/// Render the TUI menu for the given request.
/// Uses alternate screen + raw mode. Returns TerminalResponse.
/// If terminal does not support raw mode, caller must have checked before calling.
pub fn render_tui(request: &InteractionRequest, theme_str: &str) -> TerminalResponse {
    match render_tui_inner(request, theme_str) {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("[sy-interact] TUI render error: {e}");
            TerminalResponse::cancelled("tui")
        }
    }
}

fn render_tui_inner(request: &InteractionRequest, theme_str: &str) -> std::io::Result<TerminalResponse> {
    enable_raw_mode()?;
    let mut stderr = std::io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::from_str(theme_str);
    let default_ids = request.default_option_ids.clone().unwrap_or_default();
    let mut state = TuiState::new(&request.options, request.presentation.show_details_by_default);
    state.apply_defaults(&request.options, &default_ids, &request.selection_mode);

    let needs_comment = matches!(request.comment_mode, CommentMode::Required | CommentMode::Optional);
    let is_multi = matches!(request.selection_mode, SelectionMode::MultiSelect);

    let result = run_event_loop(&mut terminal, request, &theme, &mut state, needs_comment, is_multi);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    request: &InteractionRequest,
    theme: &Theme,
    state: &mut TuiState,
    needs_comment: bool,
    is_multi: bool,
) -> std::io::Result<TerminalResponse> {
    loop {
        terminal.draw(|frame| draw_frame(frame, request, theme, state, needs_comment, is_multi))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            // Global cancel: Ctrl-C / Esc / q
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(TerminalResponse::cancelled("tui"));
            }
            if key.code == KeyCode::Esc || (key.code == KeyCode::Char('q') && state.focus == FocusPanel::Options) {
                return Ok(TerminalResponse::cancelled("tui"));
            }

            match state.focus {
                FocusPanel::Options => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => state.cursor_up(),
                    KeyCode::Down | KeyCode::Char('j') => state.cursor_down(),
                    KeyCode::Char(' ') if is_multi => state.toggle_select(),
                    KeyCode::Tab => {
                        state.show_details = !state.show_details;
                        if needs_comment {
                            state.focus = FocusPanel::Comment;
                        }
                    }
                    KeyCode::Enter => {
                        if is_multi {
                            // multi: Enter confirms all selected
                            if state.selected.is_empty() { continue; }
                        } else {
                            state.confirm_single();
                        }
                        if needs_comment && state.focus == FocusPanel::Options {
                            state.focus = FocusPanel::Comment;
                        } else {
                            return Ok(build_response(request, state));
                        }
                    }
                    _ => {}
                },
                FocusPanel::Comment => match key.code {
                    KeyCode::Tab => state.focus = FocusPanel::Options,
                    KeyCode::Enter => return Ok(build_response(request, state)),
                    KeyCode::Backspace => { state.comment.pop(); }
                    KeyCode::Char(c) => state.comment.push(c),
                    _ => {}
                },
            }
        }
    }
}

fn draw_frame(
    frame: &mut Frame,
    request: &InteractionRequest,
    theme: &Theme,
    state: &TuiState,
    needs_comment: bool,
    is_multi: bool,
) {
    let area = frame.area();

    // Options list height: N rows + 2 borders (no extra comment row)
    let n_options = request.options.len() as u16;
    let options_height = n_options + 2;

    // Layout: title | options | [details] | status
    let mut constraints = vec![
        Constraint::Length(3),
        Constraint::Length(options_height),
    ];
    if state.show_details { constraints.push(Constraint::Length(4)); }
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut chunk_idx = 0;

    // Title
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(&request.title, theme.title())]))
            .block(Block::default().borders(Borders::BOTTOM))
            .wrap(Wrap { trim: true }),
        chunks[chunk_idx],
    );
    chunk_idx += 1;

    // Options list — requires_comment option shows inline input when selected
    let items: Vec<ListItem> = request.options.iter().enumerate().map(|(i, opt)| {
        let is_cursor = state.cursor == i;
        let is_input = opt.requires_comment && is_cursor;

        let (line, style) = if opt.requires_comment && !is_cursor {
            // Placeholder: dim/gray, shows option label as hint
            let text = format!("    {}…", opt.label);
            (Line::from(text), theme.status_bar())
        } else if is_input {
            // Active input row: show typed text + cursor
            let content = if state.comment.is_empty() {
                format!("  > |")
            } else {
                format!("  > {}|", state.comment)
            };
            (Line::from(content), theme.focused_row())
        } else if is_multi {
            let prefix = if state.is_selected(i) { "[x] " } else { "[ ] " };
            let suffix = if opt.recommended { " (recommended)" } else { "" };
            let danger = if opt.danger { " [!]" } else { "" };
            let style = if is_cursor { theme.focused_row() } else if opt.danger { theme.danger_row() } else { theme.normal_row() };
            (Line::from(format!("{prefix}{}{danger}{suffix}", opt.label)), style)
        } else {
            let prefix = if is_cursor { "  > " } else { "    " };
            let suffix = if opt.recommended { " (recommended)" } else { "" };
            let danger = if opt.danger { " [!]" } else { "" };
            let style = if is_cursor { theme.focused_row() } else if opt.danger { theme.danger_row() } else { theme.normal_row() };
            (Line::from(format!("{prefix}{}{danger}{suffix}", opt.label)), style)
        };
        ListItem::new(line).style(style)
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.cursor));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Options "));
    frame.render_stateful_widget(list, chunks[chunk_idx], &mut list_state);
    chunk_idx += 1;

    // Details panel
    if state.show_details {
        let detail_text = request.options.get(state.cursor)
            .and_then(|o| o.description.as_deref())
            .unwrap_or("No description.");
        frame.render_widget(
            Paragraph::new(detail_text)
                .block(Block::default().borders(Borders::ALL).title(" Details "))
                .wrap(Wrap { trim: true }),
            chunks[chunk_idx],
        );
        chunk_idx += 1;
    }

    // Status bar
    let cursor_requires_comment = request.options.get(state.cursor)
        .map(|o| o.requires_comment).unwrap_or(false);
    let help = if is_multi {
        "↑/↓ move  Space select  Enter confirm  q/Esc cancel"
    } else if cursor_requires_comment {
        "↑/↓ move  type to input  Enter confirm  q/Esc cancel"
    } else {
        "↑/↓ move  Enter select  q/Esc cancel"
    };
    frame.render_widget(
        Paragraph::new(help).style(theme.status_bar()),
        chunks[chunk_idx],
    );
}

fn build_response(request: &InteractionRequest, state: &TuiState) -> TerminalResponse {
    let ids: Vec<String> = state.selected_ids(&request.options)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let comment = if state.comment.trim().is_empty() { None } else { Some(state.comment.trim().to_string()) };
    TerminalResponse {
        status: ResponseStatus::Answered,
        selected_option_ids: ids,
        comment,
        presenter_mode: "tui".to_string(),
    }
}

/// Render the TUI menu inline (no alternate screen).
/// Uses ratatui Viewport::Inline so the menu appears in the current scroll position.
pub fn render_tui_inline(request: &InteractionRequest, theme_str: &str) -> TerminalResponse {
    match render_tui_inline_inner(request, theme_str) {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("[sy-interact] inline TUI render error: {e}");
            TerminalResponse::cancelled("tui_inline")
        }
    }
}

fn render_tui_inline_inner(request: &InteractionRequest, theme_str: &str) -> std::io::Result<TerminalResponse> {
    // Compute viewport height: title(3) + options(N+2) + comment(3 if any option requires it) + status(2)
    let n_opts = request.options.len() as u16;
    let any_requires_comment = request.options.iter().any(|o| o.requires_comment);
    let needs_comment_height = any_requires_comment
        || matches!(request.comment_mode, CommentMode::Optional | CommentMode::Required);
    let viewport_height = (3 + (n_opts + 2) + if needs_comment_height { 3 } else { 0 } + 2).min(24);

    enable_raw_mode()?;
    // Drain stale buffered events (e.g. the Enter that launched this process)
    // before entering the interactive loop.
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read()?;
    }
    let stderr = std::io::stderr();
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions { viewport: Viewport::Inline(viewport_height) },
    )?;

    let theme = Theme::from_str(theme_str);
    let is_multi = matches!(request.selection_mode, crate::interaction::schema::SelectionMode::MultiSelect);
    let needs_comment = matches!(request.comment_mode, CommentMode::Optional | CommentMode::Required);
    let any_requires_comment = request.options.iter().any(|o| o.requires_comment);
    let comment_reachable = needs_comment || any_requires_comment;
    let mut state = TuiState::new(&request.options, false);

    let result = loop {
        let cursor_requires_comment = request.options.get(state.cursor)
            .map(|o| o.requires_comment)
            .unwrap_or(false);
        // Auto-sync focus: if cursor is on a requires_comment option, focus = Comment
        if cursor_requires_comment {
            state.focus = FocusPanel::Comment;
        } else {
            state.focus = FocusPanel::Options;
        }

        terminal.draw(|frame| draw_frame(frame, request, &theme, &state, needs_comment, is_multi))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }
                match (key.code, key.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL)
                    | (KeyCode::Esc, _) => {
                        break TerminalResponse::cancelled("tui_inline");
                    }
                    (KeyCode::Char('q'), KeyModifiers::NONE) if state.focus == FocusPanel::Options => {
                        break TerminalResponse::cancelled("tui_inline");
                    }
                    (KeyCode::Up, _) => {
                        state.cursor_up();
                    }
                    (KeyCode::Down, _) => {
                        state.cursor_down();
                    }
                    (KeyCode::Char(' '), _) if is_multi && state.focus == FocusPanel::Options => state.toggle_select(),
                    (KeyCode::Backspace, _) if state.focus == FocusPanel::Comment => { state.comment.pop(); }
                    (KeyCode::Char(c), m) if state.focus == FocusPanel::Comment
                        && !m.contains(KeyModifiers::CONTROL)
                        && !m.contains(KeyModifiers::ALT) => state.comment.push(c),
                    (KeyCode::Enter, _) => {
                        if is_multi {
                            if !state.selected.is_empty() {
                                break build_response(request, &state);
                            }
                        } else {
                            state.confirm_single();
                            break build_response(request, &state);
                        }
                    }
                    _ => {}
                }
            }
        }
    };

    disable_raw_mode()?;
    let mut stderr2 = std::io::stderr();
    execute!(stderr2, crossterm::cursor::MoveToNextLine(1))?;

    Ok(result)
}
