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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
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

    // Layout: title | options | [details] | [comment] | status
    let mut constraints = vec![
        Constraint::Length(3), // title
        Constraint::Min(3),    // options
    ];
    if state.show_details { constraints.push(Constraint::Length(4)); }
    if needs_comment { constraints.push(Constraint::Length(3)); }
    constraints.push(Constraint::Length(1)); // status bar

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut chunk_idx = 0;

    // Title
    let title_text = Line::from(vec![
        Span::styled(&request.title, theme.title()),
    ]);
    frame.render_widget(
        Paragraph::new(title_text)
            .block(Block::default().borders(Borders::BOTTOM))
            .wrap(Wrap { trim: true }),
        chunks[chunk_idx],
    );
    chunk_idx += 1;

    // Options list
    let items: Vec<ListItem> = request.options.iter().enumerate().map(|(i, opt)| {
        let prefix = if is_multi {
            if state.is_selected(i) { "[x] " } else { "[ ] " }
        } else {
            if state.cursor == i { "  > " } else { "    " }
        };
        let suffix = if opt.recommended { " (recommended)" } else { "" };
        let danger_mark = if opt.danger { " [!]" } else { "" };
        let line = Line::from(format!("{prefix}{}{danger_mark}{suffix}", opt.label));
        let style = if i == state.cursor {
            theme.focused_row()
        } else if opt.danger {
            theme.danger_row()
        } else {
            theme.normal_row()
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
        let selected_opt = request.options.get(state.cursor);
        let detail_text = selected_opt
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

    // Comment input
    if needs_comment {
        let label = match request.comment_mode {
            CommentMode::Required => " Comment (required) ",
            _ => " Comment (optional) ",
        };
        let comment_style = if state.focus == FocusPanel::Comment {
            theme.focused_row()
        } else {
            theme.normal_row()
        };
        frame.render_widget(
            Paragraph::new(state.comment.as_str())
                .style(comment_style)
                .block(Block::default().borders(Borders::ALL).title(label)),
            chunks[chunk_idx],
        );
        chunk_idx += 1;
    }

    // Status bar
    let help = if is_multi {
        "↑/↓ move  Space select  Enter confirm  Tab details  q/Esc cancel"
    } else {
        "↑/↓ move  Enter select  Tab details  q/Esc cancel"
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
