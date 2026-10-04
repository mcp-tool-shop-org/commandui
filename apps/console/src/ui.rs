//! Console UI renderer — multi-session aware.
//!
//! Renders the active session's state. Status bar shows session indicator.

use crate::model::{CommandProposal, InputMode, Model, SessionState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Restores the host terminal when dropped, including after a render panic.
/// Armed for the whole `App::run` so raw mode cannot stay on.
pub struct TerminalRestoreGuard;

impl TerminalRestoreGuard {
    pub fn arm() -> Self {
        Self
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        // Raw Play can leave mouse, focus and paste reporting on in the host;
        // turn them off so a panic does not leave escape garbage on every click.
        // All of these are idempotent.
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableFocusChange,
            crossterm::event::DisableBracketedPaste,
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show,
        );
    }
}

pub fn render(frame: &mut Frame, model: &mut Model) {
    match model.input_mode {
        InputMode::Shell => render_shell_layout(frame, model),
        InputMode::Ask => render_ask_layout(frame, model),
        InputMode::Review => render_review_layout(frame, model),
        InputMode::Switcher => render_switcher_layout(frame, model),
        InputMode::RawPlay => {} // Game owns the terminal — no Console rendering
    }
}

// ---- Layouts ----

fn render_shell_layout(frame: &mut Frame, model: &Model) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_status_bar(frame, chunks[0], model);
    render_terminal_pane(frame, chunks[1], model);
    render_shell_footer(frame, chunks[2], model);

    if model.show_help {
        render_help_overlay(frame, chunks[1]);
    }
}

fn render_ask_layout(frame: &mut Frame, model: &Model) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_status_bar(frame, chunks[0], model);
    render_terminal_pane(frame, chunks[1], model);
    render_composer(frame, chunks[2], model);
}

fn render_review_layout(frame: &mut Frame, model: &mut Model) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_status_bar(frame, chunks[0], model);
    render_terminal_pane(frame, chunks[1], model);
    render_review_panel(frame, chunks[2], model);
    render_review_footer(frame, chunks[3], model);
}

fn render_switcher_layout(frame: &mut Frame, model: &mut Model) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(3),   // terminal pane (dimmed) + overlay
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    render_status_bar(frame, chunks[0], model);

    // Render the terminal pane as background, then overlay on top
    render_terminal_pane(frame, chunks[1], model);
    render_run_selector_overlay(frame, chunks[1], model);

    render_switcher_footer(frame, chunks[2]);
}

// ---- Status bar (session-aware) ----

fn render_status_bar(frame: &mut Frame, area: Rect, model: &Model) {
    let active = model.active_session();

    // State label: only shown for non-Active states (exec display handles Active)
    let (state_label, state_color) = match active.map(|s| &s.session_state) {
        Some(SessionState::Booting) => (Some("BOOT"), Color::Yellow),
        Some(SessionState::Active) => (None, Color::Green), // exec display covers this
        Some(SessionState::Closed) => (Some("DONE"), Color::Red),
        Some(SessionState::Error(_)) => (Some("ERROR"), Color::Red),
        None => (Some("NO SESSION"), Color::Red),
    };

    let exec_color = match active.map(|s| s.exec_state.as_str()) {
        Some("ready") => Color::Green,
        Some("running") | Some("userRunning") => Color::Cyan,
        Some("interrupting") | Some("booting") => Color::Yellow,
        _ => Color::Red,
    };

    let mode_label = match model.input_mode {
        InputMode::Shell => "SHELL",
        InputMode::Ask => "ASK",
        InputMode::Review => "REVIEW",
        InputMode::Switcher => "RUNS",
        InputMode::RawPlay => "RAW PLAY", // shouldn't render, but complete the match
    };
    let mode_color = match model.input_mode {
        InputMode::Shell => Color::Cyan,
        InputMode::Ask => Color::Magenta,
        InputMode::Review => Color::Yellow,
        InputMode::Switcher => Color::Cyan,
        InputMode::RawPlay => Color::Red,
    };

    let cwd_display = active
        .and_then(|s| s.cwd.as_deref())
        .unwrap_or("...");

    // Session indicator: [1/3] or [1/1]
    let session_indicator = if model.session_count() == 0 {
        "[0/0]".to_string()
    } else {
        format!("[{}/{}]", model.active_index + 1, model.session_count())
    };

    let session_label = active.map(|s| s.label.as_str()).unwrap_or("—");

    let mut spans = vec![
        Span::styled(
            " CommandUI Console ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!(" {mode_label} "),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            session_indicator,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            session_label,
            Style::default().fg(Color::White),
        ),
    ];

    if let Some(label) = state_label {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            label,
            Style::default().fg(state_color).add_modifier(Modifier::BOLD),
        ));
    }

    if active.map_or(false, |s| s.session_state == SessionState::Active) {
        let exec_display = match active.map(|s| s.exec_state.as_str()) {
            Some("ready") => "idle",
            Some("interrupting") => "stopping",
            Some("userRunning") => "program running",
            Some("booting") => "starting",
            Some(other) => other,
            None => "?",
        };
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            exec_display,
            Style::default().fg(exec_color),
        ));
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(cwd_display, Style::default().fg(Color::Blue)));
    }

    let status = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(status, area);
}

// ---- Terminal pane (active session) ----

fn render_terminal_pane(frame: &mut Frame, area: Rect, model: &Model) {
    let active = model.active_session();
    let is_ready = active.map_or(false, |s| s.is_ready());

    let title = match active.map(|s| &s.session_state) {
        Some(SessionState::Booting) => " Terminal (starting...) ",
        Some(SessionState::Active) => " Terminal ",
        Some(SessionState::Closed) => " Terminal (done) ",
        Some(SessionState::Error(_)) => " Terminal (error) ",
        None => " No Session ",
    };

    let border_color = match active.map(|s| &s.session_state) {
        Some(SessionState::Active) => Color::DarkGray,
        Some(SessionState::Booting) => Color::Yellow,
        _ => Color::Red,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let inner = block.inner(area);
    let inner_height = inner.height as usize;
    let inner_width = inner.width as usize;

    let visible_lines: Vec<Line> = if let Some(session) = active {
        if let SessionState::Error(msg) = &session.session_state {
            wrap_rows(msg, inner_width)
                .into_iter()
                .take(inner_height)
                .map(|row| Line::from(Span::styled(row, Style::default().fg(Color::Red))))
                .collect()
        } else if session.terminal_lines.is_empty()
            && session.line_remainder.is_empty()
            && !is_ready
        {
            vec![Line::from(Span::styled(
                "Starting shell...",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            let mut lines: Vec<Line> = Vec::new();
            let marker = dropped_marker(session);
            if let Some(text) = marker {
                lines.push(Line::from(Span::styled(
                    text,
                    Style::default().fg(Color::DarkGray),
                )));
            }
            let room = inner_height.saturating_sub(lines.len());
            lines.extend(
                terminal_window_rows(session, inner_width, room)
                    .into_iter()
                    .map(Line::from),
            );
            lines
        }
    } else {
        let mut welcome = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  CommandUI Console — terminal shell with sidecar AI",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled("    ^T  Ask the AI           ^G  Raw Play (fullscreen)", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("    ^S  Switch runs           ^N  New session", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("    F1  Help                  ^Q  Quit", Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from(Span::styled(
                "  Press ^N to start a session.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        if let Some(err) = model.create_error.as_deref() {
            welcome.push(Line::from(""));
            welcome.push(Line::from(Span::styled(
                format!("  {err}"),
                Style::default().fg(Color::Red),
            )));
        }
        welcome
    };

    // Wrapping is done above, so the newest rows are the ones kept.
    let paragraph = Paragraph::new(visible_lines).block(block);
    frame.render_widget(paragraph, area);
}

/// One-line scrollback-loss indicator, when anything was discarded.
fn dropped_marker(session: &crate::model::SessionModel) -> Option<String> {
    match (session.lines_dropped, session.bytes_dropped) {
        (0, 0) => None,
        (lines, 0) => Some(format!("[{lines} lines dropped]")),
        (0, bytes) => Some(format!("[{bytes} bytes dropped]")),
        (lines, bytes) => Some(format!("[{lines} lines, {bytes} bytes dropped]")),
    }
}

/// Display width of a string in terminal cells.
pub(crate) fn display_width(text: &str) -> usize {
    Span::raw(text).width()
}

fn cell_width(c: char) -> usize {
    Span::raw(c.to_string()).width()
}

/// Break one logical line into rows no wider than `cols` cells. Tabs expand to
/// 8-column stops and other controls are dropped. An empty line is one empty row.
pub(crate) fn wrap_rows(line: &str, cols: usize) -> Vec<String> {
    if cols == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut cur = String::new();
    let mut used = 0usize;
    for c in line.chars() {
        if c != '\t' && c.is_control() {
            continue;
        }
        let (ch, reps) = if c == '\t' { (' ', 8 - used % 8) } else { (c, 1) };
        for _ in 0..reps {
            let w = cell_width(ch);
            if w == 0 {
                if !cur.is_empty() {
                    cur.push(ch);
                }
                continue;
            }
            if w > cols {
                continue;
            }
            if used + w > cols {
                rows.push(std::mem::take(&mut cur));
                used = 0;
            }
            cur.push(ch);
            used += w;
        }
    }
    rows.push(cur);
    rows
}

/// The newest `room` rows of the session, ending at the anchored line.
/// `scroll_offset` counts logical lines from the tail; the open line (the
/// text after the last newline) shows only at the live end.
fn terminal_window_rows(
    session: &crate::model::SessionModel,
    cols: usize,
    room: usize,
) -> Vec<String> {
    if room == 0 || cols == 0 {
        return Vec::new();
    }
    let total = session.terminal_lines.len();
    let end = total.saturating_sub(session.scroll_offset);
    let mut reversed: Vec<String> = Vec::new();
    if session.scroll_offset == 0 && !session.line_remainder.is_empty() {
        reversed.extend(wrap_rows(&session.line_remainder, cols).into_iter().rev());
    }
    for line in session.terminal_lines[..end].iter().rev() {
        if reversed.len() >= room {
            break;
        }
        reversed.extend(wrap_rows(line, cols).into_iter().rev());
    }
    reversed.truncate(room);
    reversed.reverse();
    reversed
}

/// How many logical lines a page of `page_rows` visual rows covers, counted
/// back from the current window end. At least one.
pub(crate) fn page_lines(
    session: &crate::model::SessionModel,
    cols: usize,
    page_rows: usize,
) -> usize {
    let end = session
        .terminal_lines
        .len()
        .saturating_sub(session.scroll_offset);
    let mut rows = 0usize;
    let mut count = 0usize;
    for line in session.terminal_lines[..end].iter().rev() {
        if rows >= page_rows.max(1) {
            break;
        }
        rows += wrap_rows(line, cols.max(1)).len();
        count += 1;
    }
    count.max(1)
}

// ---- Shell footer ----

fn render_shell_footer(frame: &mut Frame, area: Rect, model: &Model) {
    let mut spans = vec![
        Span::styled(
            " SHELL ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let is_running = model
        .active_session()
        .map_or(false, |s| s.has_running_command());

    if let Some(prompt) = model.confirm_prompt() {
        spans.push(Span::styled(
            format!("  {prompt}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    } else if model.can_accept_input() {
        if is_running {
            spans.push(Span::raw("  ^C Stop  ^G Raw Play  ^S Runs  F1 Help  ^Q Quit"));
        } else {
            spans.push(Span::raw("  ^T Ask  ^G Raw Play  ^S Runs  F1 Help  ^Q Quit"));
        }
    } else if model.active_session().is_some_and(|s| s.boot_is_slow()) {
        spans.push(Span::styled(
            "  Still starting. ^R sends Enter, ^W closes, ^N New",
            Style::default().fg(Color::Yellow),
        ));
    } else {
        spans.push(Span::raw("  ^N New  ^Q Quit"));
    }

    if let Some(s) = model.active_session() {
        if s.scroll_offset > 0 {
            spans.push(Span::styled(
                format!("  [{} below]", s.scroll_offset),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    if model.confirm_prompt().is_none() {
        if let Some(ref err) = model.status_line {
            spans.push(Span::styled(
                format!("  {err}"),
                Style::default().fg(Color::Red),
            ));
        } else if let Some(err) = model.service_errors.last() {
            let text = if model.service_errors.len() > 1 {
                format!("  [{} errors] {}", model.service_errors.len(), err)
            } else {
                format!("  {err}")
            };
            spans.push(Span::styled(text, Style::default().fg(Color::Red)));
        }
    }

    let footer = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

// ---- Composer ----

fn render_composer(frame: &mut Frame, area: Rect, model: &Model) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(if let Some(prompt) = model.confirm_prompt() {
            format!(" {prompt} ")
        } else if model.planner_busy {
            " Ask (generating...) ".to_string()
        } else {
            " Ask — describe what you want ".to_string()
        });

    let text = &model.composer_text;
    let cursor = model.composer_cursor;

    let content = if model.planner_busy {
        Line::from(Span::styled(
            "Generating proposal...",
            Style::default().fg(Color::Yellow),
        ))
    } else if let Some(ref err) = model.planner_error {
        Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::styled(err.as_str(), Style::default().fg(Color::Red)),
            Span::raw("  (type to try again)"),
        ])
    } else if text.is_empty() {
        Line::from(Span::styled(
            "Type your intent and press Enter... (Esc to cancel)",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        // Scroll a window of the text so the cursor cell is always inside the pane.
        let width = usize::from(area.width.saturating_sub(2)).max(1);
        let chars: Vec<char> = text.chars().collect();
        let cursor_idx = text[..cursor].chars().count();
        let start = (cursor_idx + 1).saturating_sub(width);
        let end = (start + width).min(chars.len());
        let window = &chars[start.min(chars.len())..end];
        let rel = cursor_idx - start;
        let before: String = window.iter().take(rel).collect();
        let cursor_char = window.get(rel).copied().unwrap_or(' ');
        let after: String = window.iter().skip(rel + 1).collect();

        Line::from(vec![
            Span::raw(before),
            Span::styled(
                cursor_char.to_string(),
                Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw(after),
        ])
    };

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

// ---- Review panel ----

fn render_review_panel(frame: &mut Frame, area: Rect, model: &mut Model) {
    let command_lines = model
        .current_proposal
        .as_ref()
        .map(|proposal| visible_command_lines(&proposal.command))
        .unwrap_or_default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    let meta = review_meta_lines(model, inner.width as usize);
    let (command_rows, meta_rows) = split_review_rows(inner.height as usize, meta.len());
    model.review_rows = command_rows;
    model.review_cols = inner.width as usize;

    let clipped = command_is_clipped(
        &command_lines,
        model.review_scroll,
        model.review_scroll_x,
        command_rows,
        inner.width as usize,
    );
    let title = review_title(model, clipped);
    let block = block.title(title);

    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(command_rows as u16),
            Constraint::Length(meta_rows as u16),
        ])
        .split(inner);

    let max_y = command_lines.len().saturating_sub(command_rows);
    let widest = widest_line(&command_lines);
    let max_x = widest.saturating_sub(inner.width as usize);
    let scroll_y = model.review_scroll.min(max_y);
    let scroll_x = model.review_scroll_x.min(max_x);
    let command_text: Vec<Line> = command_lines
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                line.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    let command = Paragraph::new(command_text).scroll((scroll_u16(scroll_y), scroll_u16(scroll_x)));
    frame.render_widget(command, chunks[0]);

    let meta_paragraph = Paragraph::new(meta);
    frame.render_widget(meta_paragraph, chunks[1]);
}

fn review_title(model: &Model, clipped: bool) -> String {
    let owner = if let Some(ref owner_id) = model.proposal_session_id {
        let owner_label = model
            .sessions
            .iter()
            .find(|s| s.id == *owner_id)
            .map(|s| s.label.as_str())
            .unwrap_or("?");
        format!(" Review Proposal → {owner_label}")
    } else {
        " Review Proposal".to_string()
    };
    // The shared planner returns a mock command only when the model call failed.
    let owner = if model
        .current_proposal
        .as_ref()
        .is_some_and(|p| p.source == "mock")
    {
        format!("{owner} (mock: model call failed)")
    } else {
        owner
    };
    if clipped {
        format!("{owner} (clipped) ")
    } else {
        format!("{owner} ")
    }
}

fn render_review_footer(frame: &mut Frame, area: Rect, model: &Model) {
    let mut spans = vec![
        Span::styled(
            " REVIEW ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(prompt) = model.confirm_prompt() {
        spans.push(Span::styled(
            format!("  {prompt}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::raw(
            "  ↑↓ scroll  ←→ wide  c confirm  Enter approve  Esc cancel  ^Q Quit",
        ));
        if let Some(ref err) = model.status_line {
            spans.push(Span::styled(
                format!("  {err}"),
                Style::default().fg(Color::Red),
            ));
        } else if let Some(err) = model.service_errors.last() {
            let text = if model.service_errors.len() > 1 {
                format!("  [{} errors] {}", model.service_errors.len(), err)
            } else {
                format!("  {err}")
            };
            spans.push(Span::styled(text, Style::default().fg(Color::Red)));
        }
    }

    let footer = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

/// Inner rows of the fixed 10-row review pane (borders take two).
pub(crate) const REVIEW_INNER_ROWS: usize = 8;

/// Quoted, control-visible lines for the command the shell will run.
/// Newlines are separate rows. Other controls are escapes inside the quotes.
pub(crate) fn visible_command_lines(command: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (idx, ch) in command.char_indices() {
        if ch == '\n' {
            lines.push(quote_segment(&command[start..idx]));
            start = idx + ch.len_utf8();
        }
    }
    if start < command.len() || command.is_empty() || command.ends_with('\n') {
        lines.push(quote_segment(&command[start..]));
    }
    lines
}

fn quote_segment(segment: &str) -> String {
    format!("$ '{}'", escape_segment(segment))
}

/// Characters that change how text is drawn or ordered without being visible:
/// Unicode category Cf (bidi overrides and isolates, zero-width characters,
/// joiners) plus the line and paragraph separators U+2028 and U+2029.
/// Written out because the standard library has no category lookup.
fn is_invisible_format(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

fn escape_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    let mut chars = segment.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // A backslash is doubled only where it could be read as one of this
            // pane's own escapes, so C:\Users shows as typed.
            '\\' if matches!(chars.peek(), Some('r' | 't' | 'x' | 'u' | '\'' | '\\')) => {
                out.push_str("\\\\")
            }
            '\\' => out.push('\\'),
            '\'' => out.push_str("\\'"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || is_invisible_format(c) => {
                let code = u32::from(c);
                if code <= 0xff {
                    out.push_str(&format!("\\x{code:02x}"));
                } else {
                    out.push_str(&format!("\\u{{{code:x}}}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn review_meta_rows(model: &Model) -> usize {
    let width = if model.review_cols == 0 {
        model.pane_cols.max(1) as usize
    } else {
        model.review_cols
    };
    review_meta_lines(model, width).len()
}

/// Greedy word wrap to `cols` cells. A word wider than a row is cut by cell.
fn wrap_words(text: &str, cols: usize) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if display_width(word) > cols {
            if !cur.is_empty() {
                rows.push(std::mem::take(&mut cur));
            }
            let mut pieces = wrap_rows(word, cols);
            cur = pieces.pop().unwrap_or_default();
            rows.extend(pieces);
        } else if cur.is_empty() {
            cur = word.to_string();
        } else if display_width(&cur) + 1 + display_width(word) <= cols {
            cur.push(' ');
            cur.push_str(word);
        } else {
            rows.push(std::mem::replace(&mut cur, word.to_string()));
        }
    }
    rows.push(cur);
    rows
}

/// A meta line: a bold label, then text. Wrapped to `width` cells so nothing
/// is clipped on the right; the label keeps its style on the first row only.
fn wrapped_meta(
    label: &str,
    label_style: Style,
    body: &str,
    body_style: Style,
    width: usize,
) -> Vec<Line<'static>> {
    let label_chars = label.chars().count();
    wrap_words(&format!("{label}{body}"), width.max(1))
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == 0 && label_chars > 0 && row.chars().count() >= label_chars {
                let head: String = row.chars().take(label_chars).collect();
                let tail: String = row.chars().skip(label_chars).collect();
                Line::from(vec![
                    Span::styled(head, label_style),
                    Span::styled(tail, body_style),
                ])
            } else {
                Line::from(Span::styled(row, body_style))
            }
        })
        .collect()
}

/// The review meta block, in priority order. The pane drops rows from the end
/// when it is short, so what guards the approve comes first: the safety flags
/// (non-ASCII first, the homoglyph warning), the confirmation notice, and the
/// error row (always reserved). Risk, directory and explanation follow.
fn review_meta_lines(model: &Model, width: usize) -> Vec<Line<'static>> {
    let Some(proposal) = model.current_proposal.as_ref() else {
        return vec![Line::from(Span::styled(
            "No proposal to review.",
            Style::default().fg(Color::DarkGray),
        ))];
    };

    let red = Style::default().fg(Color::Red);
    let mut lines: Vec<Line<'static>> = Vec::new();

    let mut flags: Vec<&'static str> = Vec::new();
    if !proposal.command.is_ascii() {
        flags.push("non-ASCII characters");
    }
    flags.extend(safety_flags(proposal));
    if !flags.is_empty() {
        lines.extend(wrapped_meta("Flags: ", red, &flags.join(", "), red, width));
    }
    if proposal.requires_confirmation {
        let label = if model.proposal_confirmed {
            "Confirmation: confirmed"
        } else {
            "Confirmation required — press c, then Enter"
        };
        lines.extend(wrapped_meta(
            "",
            Style::default(),
            label,
            Style::default().fg(Color::Yellow),
            width,
        ));
    }
    // A program in the foreground blocks the approve; say so before the user presses it.
    if let Some(msg) = model
        .proposal_session_id
        .as_deref()
        .and_then(|id| model.sessions.iter().find(|s| s.id == id))
        .and_then(|s| s.busy_message())
    {
        lines.extend(wrapped_meta(
            "Foreground: ",
            red,
            &format!("{msg} Approve is refused until the prompt returns."),
            red,
            width,
        ));
    }
    let err = model.review_error.as_deref().unwrap_or("");
    lines.extend(wrapped_meta("", Style::default(), err, red, width));

    lines.push(risk_line(proposal));
    let target = proposal.cwd.as_deref().filter(|c| !c.is_empty()).unwrap_or("unknown");
    lines.extend(wrapped_meta(
        "Runs in: ",
        Style::default().add_modifier(Modifier::BOLD),
        target,
        Style::default(),
        width,
    ));
    lines.extend(wrapped_meta(
        "Explanation: ",
        Style::default().add_modifier(Modifier::BOLD),
        proposal.explanation.as_str(),
        Style::default(),
        width,
    ));
    lines
}

fn risk_line(proposal: &CommandProposal) -> Line<'static> {
    let risk_color = match proposal.risk.as_str() {
        "low" => Color::Green,
        "medium" => Color::Yellow,
        "high" => Color::Red,
        _ => Color::White,
    };
    Line::from(vec![
        Span::styled("Risk:    ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {} ", proposal.risk.to_uppercase()),
            Style::default()
                .fg(Color::Black)
                .bg(risk_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  confidence: {:.0}%", proposal.confidence * 100.0)),
        Span::raw(format!("  source: {}", proposal.source)),
    ])
}

fn safety_flags(proposal: &CommandProposal) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if proposal.destructive {
        flags.push("DESTRUCTIVE");
    }
    if proposal.escalates_privileges {
        flags.push("PRIVILEGE_ESCALATION");
    }
    if proposal.touches_network {
        flags.push("NETWORK_ACCESS");
    }
    flags
}

/// Split the review pane's inner rows into the command viewport and the meta block.
/// The command keeps at least one row when the pane is not empty.
pub(crate) fn split_review_rows(inner_rows: usize, meta_rows: usize) -> (usize, usize) {
    if inner_rows == 0 {
        return (0, 0);
    }
    let meta = meta_rows.min(inner_rows.saturating_sub(1));
    let command = inner_rows.saturating_sub(meta);
    (command, meta)
}

pub(crate) fn review_window(model: &Model) -> (usize, usize) {
    let rows = if model.review_rows == 0 {
        split_review_rows(REVIEW_INNER_ROWS, review_meta_rows(model)).0
    } else {
        model.review_rows
    };
    let cols = if model.review_cols == 0 {
        model.pane_cols.max(1) as usize
    } else {
        model.review_cols
    };
    (rows, cols)
}

/// True while any command row or the right edge is outside the review viewport.
pub(crate) fn command_is_clipped(
    lines: &[String],
    scroll_y: usize,
    scroll_x: usize,
    rows: usize,
    cols: usize,
) -> bool {
    if lines.is_empty() {
        return false;
    }
    if rows == 0 || cols == 0 {
        return true;
    }
    // The widget applies a u16 offset. A model scroll past that is not what is drawn.
    if scroll_y > usize::from(u16::MAX) || scroll_x > usize::from(u16::MAX) {
        return true;
    }
    let tail_hidden = scroll_y.saturating_add(rows) < lines.len();
    let right_hidden = scroll_x.saturating_add(cols) < widest_line(lines);
    tail_hidden || right_hidden
}

/// Widest line in terminal cells, the unit the Paragraph scrolls and clips by.
pub(crate) fn widest_line(lines: &[String]) -> usize {
    lines.iter().map(|line| display_width(line)).max().unwrap_or(0)
}

fn scroll_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

// ---- Run selector overlay ----

fn render_run_selector_overlay(frame: &mut Frame, area: Rect, model: &mut Model) {
    // Center the overlay in the terminal pane area. Never wider or taller than the frame.
    let want_h = u16::try_from(model.session_count())
        .unwrap_or(u16::MAX)
        .saturating_add(2); // +2 for border
    let overlay_area = clamped_overlay(frame.area(), area, 60, want_h);
    if overlay_area.width == 0 || overlay_area.height == 0 {
        model.switcher_start = 0;
        model.switcher_rows = 0;
        return;
    }

    // Window the rows around the cursor so the cursor row is always drawn.
    let inner_rows = usize::from(overlay_area.height.saturating_sub(2));
    let (start, shown) =
        switcher_window(model.session_count(), model.switcher_cursor, inner_rows);
    model.switcher_start = start;
    model.switcher_rows = shown;

    // Build the list
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Runs ");

    let mut lines: Vec<Line> = Vec::new();

    for (idx, session) in model
        .sessions
        .iter()
        .enumerate()
        .skip(start)
        .take(shown)
    {
        let is_cursor = idx == model.switcher_cursor;
        let is_active = idx == model.active_index;

        // State badge with color
        let (badge, badge_color) = match session.state_badge() {
            "RUNNING" => ("RUN", Color::Cyan),
            "IDLE" => ("IDLE", Color::Green),
            "BOOT" => ("BOOT", Color::Yellow),
            "STOPPING" => ("STOP", Color::Yellow),
            "DONE" => ("DONE", Color::Red),
            "ERROR" => ("ERR!", Color::Red),
            other => (other, Color::White),
        };

        // Unread marker
        let unread = if session.has_unread { " ●" } else { "  " };

        // Active marker
        let active_mark = if is_active { "►" } else { " " };

        // Number hint (1-9): position in the drawn window, which is what the key jumps to.
        let num = if idx - start < 9 {
            format!("{}", idx - start + 1)
        } else {
            " ".to_string()
        };

        let cwd_display = shorten_cwd(session.cwd.as_deref().unwrap_or("..."));

        let style = if is_cursor {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default()
        };

        let line = Line::from(vec![
            Span::styled(format!(" {active_mark} "), style),
            Span::styled(format!("{num} "), style.fg(if is_cursor { Color::Black } else { Color::DarkGray })),
            Span::styled(
                format!(" {badge:4} "),
                if is_cursor {
                    style
                } else {
                    Style::default().fg(badge_color).add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(format!(" {} ", session.label), style),
            Span::styled(cwd_display, if is_cursor { style } else { Style::default().fg(Color::Blue) }),
            Span::styled(
                unread,
                if is_cursor { style } else { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) },
            ),
        ]);

        lines.push(line);
    }

    // Clear the overlay area first (draw background)
    let bg = Paragraph::new(vec![Line::from(""); overlay_area.height as usize])
        .style(Style::default().bg(Color::Black));
    frame.render_widget(bg, overlay_area);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, overlay_area);
}

/// First row and row count to draw so `cursor` is inside the window.
pub(crate) fn switcher_window(count: usize, cursor: usize, rows: usize) -> (usize, usize) {
    let shown = rows.min(count);
    if shown == 0 {
        return (0, 0);
    }
    let cursor = cursor.min(count - 1);
    let start = if cursor < shown { 0 } else { cursor + 1 - shown };
    (start.min(count - shown), shown)
}

fn render_switcher_footer(frame: &mut Frame, area: Rect) {
    let spans = vec![
        Span::styled(
            " RUNS ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  ↑↓/jk navigate  Enter select  1-9 jump  ^N new  ^W close  Esc cancel"),
    ];

    let footer = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

// ---- Help overlay ----

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let overlay_area = clamped_overlay(frame.area(), area, 56, 18);
    if overlay_area.width == 0 || overlay_area.height == 0 {
        return;
    }

    // Background
    let bg = Paragraph::new(vec![Line::from(""); overlay_area.height as usize])
        .style(Style::default().bg(Color::Black));
    frame.render_widget(bg, overlay_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(" Help ");

    let lines = vec![
        Line::from(Span::styled(
            " Shell                  Ask",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" ^T  Ask the AI         Enter  Submit"),
        Line::from(" ^G  Raw Play           Esc    Cancel"),
        Line::from(" ^S  Runs               ^U     Clear"),
        Line::from(" ^N  New session"),
        Line::from(" ^W  Close session      Review"),
        Line::from(vec![
            Span::raw(" ^] next  ^[ prev       "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  Approve"),
        ]),
        Line::from(" ^C  Stop               Esc    Cancel"),
        Line::from(" ^R  Resync"),
        Line::from(""),
        Line::from(Span::styled(
            " Raw Play               Runs",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" ^\\  Exit Raw Play      ↑↓/jk  Navigate"),
        Line::from(" ^Q  Quit               Enter  Select"),
        Line::from("                        1-9    Jump"),
        Line::from(""),
        Line::from(Span::styled(" Esc or F1 to close", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, overlay_area);
}

/// Last 22 characters, prefixed with `...`, when the path is longer than 25
/// characters. The cut is always on a char boundary.
fn shorten_cwd(cwd: &str) -> String {
    let count = cwd.chars().count();
    if count <= 25 {
        return cwd.to_string();
    }
    let skip = count - 22;
    let tail: String = cwd.chars().skip(skip).collect();
    format!("...{tail}")
}

/// Center `want_w` x `want_h` inside `host`, then clamp so the rect cannot
/// extend past `frame`. A preferred size larger than the frame shrinks.
fn clamped_overlay(frame: Rect, host: Rect, want_w: u16, want_h: u16) -> Rect {
    let frame_right = frame.x.saturating_add(frame.width);
    let frame_bottom = frame.y.saturating_add(frame.height);
    let host_right = host.x.saturating_add(host.width).min(frame_right);
    let host_bottom = host.y.saturating_add(host.height).min(frame_bottom);
    let host_x = host.x.max(frame.x).min(host_right);
    let host_y = host.y.max(frame.y).min(host_bottom);
    let max_w = host_right.saturating_sub(host_x);
    let max_h = host_bottom.saturating_sub(host_y);
    let width = want_w.min(max_w);
    let height = want_h.min(max_h);
    let x = host_x.saturating_add(max_w.saturating_sub(width) / 2);
    let y = host_y.saturating_add(max_h.saturating_sub(height) / 2);
    Rect { x, y, width, height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cwd_truncation_stays_on_a_char_boundary() {
        let ideograph = "\u{65e5}";
        // 9 ideographs are 27 bytes. A byte index at len-22 is mid-character.
        let short = ideograph.repeat(9);
        assert!(short.len() > 25);
        assert_eq!(shorten_cwd(&short), short);

        let long = ideograph.repeat(30);
        let shown = shorten_cwd(&long);
        assert!(shown.is_char_boundary(3));
        assert!(shown.starts_with("..."));
        let tail = &shown[3..];
        assert_eq!(tail.chars().count(), 22);
        assert!(tail.chars().all(|c| c == '\u{65e5}'));
    }

    #[test]
    fn overlay_rect_cannot_extend_past_the_frame() {
        let frame = Rect::new(0, 0, 20, 8);
        let host = Rect::new(0, 1, 20, 6);
        let overlay = clamped_overlay(frame, host, 60, 18);
        assert!(overlay.width <= host.width);
        assert!(overlay.height <= host.height);
        assert!(overlay.x.saturating_add(overlay.width) <= frame.width);
        assert!(overlay.y.saturating_add(overlay.height) <= frame.height);
    }

    #[test]
    fn a_windows_path_shows_as_typed_in_the_review_line() {
        let lines = visible_command_lines("dir C:\\Users\\bob");
        assert_eq!(lines, vec!["$ 'dir C:\\Users\\bob'".to_string()]);
        // A typed backslash-r stays distinct from an escaped carriage return.
        let lines = visible_command_lines("echo \\r\r");
        assert_eq!(lines, vec!["$ 'echo \\\\r\\r'".to_string()]);
    }

    #[test]
    fn format_characters_are_escaped_in_the_review_line() {
        let lines = visible_command_lines("echo a\u{202e}b\u{200b}c\u{2028}d");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\\u{202e}"), "{}", lines[0]);
        assert!(lines[0].contains("\\u{200b}"), "{}", lines[0]);
        assert!(lines[0].contains("\\u{2028}"), "{}", lines[0]);
        assert!(!lines[0].chars().any(is_invisible_format));
    }

    #[test]
    fn visible_command_splits_newlines_and_shows_controls() {
        let command = "echo a\necho b\r\u{1b}[31m";
        let lines = visible_command_lines(command);
        assert_eq!(
            lines,
            vec![
                "$ 'echo a'".to_string(),
                "$ 'echo b\\r\\x1b[31m'".to_string(),
            ]
        );
        assert!(lines.iter().all(|line| !line.chars().any(|c| c.is_control())));
        assert_eq!(visible_command_lines("trail\n").len(), 2);
    }

    #[test]
    fn command_is_clipped_until_the_tail_and_the_right_edge_are_visible() {
        let lines: Vec<String> = (0..5).map(|i| format!("$ 'line-{i}'")).collect();
        assert!(command_is_clipped(&lines, 0, 0, 4, 80));
        assert!(!command_is_clipped(&lines, 1, 0, 4, 80));
        let wide = vec!["$ 'abcdefghijklmnopqrstuvwxyz'".to_string()];
        assert!(command_is_clipped(&wide, 0, 0, 4, 10));
        assert!(!command_is_clipped(&wide, 0, 20, 4, 10));
        assert!(command_is_clipped(&wide, 0, 0, 0, 10));
    }

    fn sample_proposal(command: &str, confirm: bool) -> CommandProposal {
        CommandProposal {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            source: "mock".to_string(),
            user_intent: "test".to_string(),
            command: command.to_string(),
            cwd: None,
            explanation: "because".to_string(),
            assumptions: vec![],
            confidence: 0.5,
            risk: "low".to_string(),
            destructive: false,
            requires_confirmation: confirm,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            expected_output: None,
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn draw_text(model: &mut Model) -> String {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, model)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn review_pane_shows_the_command_the_shell_will_run() {
        let mut model = Model::new();
        model.add_session("s1".into(), "Run A".into());
        let mut parts: Vec<String> = (0..20).map(|i| format!("line-{i:02}")).collect();
        parts[0] = "line-00\r\u{1b}".into();
        let command = parts.join("\n");
        model.set_proposal(sample_proposal(&command, true), "s1".into());
        model.input_mode = InputMode::Review;

        let shown = draw_text(&mut model);
        assert!(shown.contains("$ 'line-00"), "{shown}");
        assert!(
            !shown.contains("$ 'line-19'"),
            "tail must stay clipped until scrolled:\n{shown}"
        );
        assert!(shown.contains("\\r"), "{shown}");
        assert!(shown.contains("\\x1b"), "{shown}");
        assert!(
            shown.contains("Confirmation required"),
            "requires_confirmation must be on the pane:\n{shown}"
        );
        assert!(!shown.chars().any(|c| c == '\r' || c == '\u{1b}'));

        model.review_scroll = 30;
        let scrolled = draw_text(&mut model);
        assert!(
            scrolled.contains("$ 'line-19'"),
            "scroll must reveal the tail:\n{scrolled}"
        );
        assert!(!scrolled.contains("line-00"), "{scrolled}");
    }

    #[test]
    fn shell_footer_shows_a_session_error() {
        let mut model = Model::new();
        let idx = model.add_session("s1".into(), "A".into());
        model.sessions[idx].session_state = SessionState::Active;
        model.status_line = Some("Session not found: s1".into());
        let shown = draw_text(&mut model);
        assert!(shown.contains("Session not found: s1"), "{shown}");
    }

    fn draw_sized(model: &mut Model, cols: u16, rows: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(cols, rows);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, model)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn mark(model: &mut Model, state: SessionState, exec: &str) {
        let idx = model.add_session("s1".into(), "Run A".into());
        model.sessions[idx].session_state = state;
        model.sessions[idx].exec_state = exec.into();
        model.sessions[idx].cwd = Some("/work/project".into());
    }

    #[test]
    fn every_chrome_mode_paints_without_a_real_terminal() {
        let mut welcome = Model::new();
        let shown = draw_text(&mut welcome);
        assert!(shown.contains("Press ^N"), "{shown}");
        assert!(shown.contains("NO SESSION") || shown.contains("No Session"), "{shown}");

        for (state, exec, needle) in [
            (SessionState::Booting, "booting", "BOOT"),
            (SessionState::Active, "ready", "idle"),
            (SessionState::Active, "running", "running"),
            (SessionState::Active, "interrupting", "stopping"),
            (SessionState::Active, "desynced", "desynced"),
            (SessionState::Closed, "ready", "DONE"),
            (SessionState::Error("boom".into()), "ready", "ERROR"),
        ] {
            let mut model = Model::new();
            mark(&mut model, state, exec);
            model.sessions[0].terminal_lines = (0..40).map(|i| format!("line-{i}")).collect();
            model.sessions[0].line_remainder = "partial".into();
            model.sessions[0].scroll_offset = 3;
            model.status_line = Some("status".into());
            let shown = draw_text(&mut model);
            assert!(shown.contains(needle), "{exec} missing {needle}:\n{shown}");
        }

        let mut help = Model::new();
        mark(&mut help, SessionState::Active, "ready");
        help.show_help = true;
        let shown = draw_text(&mut help);
        assert!(shown.contains("Help"), "{shown}");

        let mut ask = Model::new();
        mark(&mut ask, SessionState::Active, "ready");
        ask.input_mode = InputMode::Ask;
        let empty = draw_text(&mut ask);
        assert!(empty.contains("Type your intent"), "{empty}");
        ask.planner_error = Some("planner down".into());
        let erred = draw_text(&mut ask);
        assert!(erred.contains("planner down"), "{erred}");
        ask.planner_error = None;
        ask.planner_busy = true;
        ask.composer_text = "list files".into();
        ask.composer_cursor = 1;
        let busy = draw_text(&mut ask);
        assert!(busy.contains("Generating"), "{busy}");
        ask.planner_busy = false;
        ask.composer_text = "日a".into();
        ask.composer_cursor = "日".len();
        let typed = draw_text(&mut ask);
        assert!(typed.contains('a'), "{typed}");

        let mut runs = Model::new();
        for (id, label, state, exec) in [
            ("s1", "A", SessionState::Booting, "booting"),
            ("s2", "B", SessionState::Active, "running"),
            ("s3", "C", SessionState::Active, "interrupting"),
            ("s4", "D", SessionState::Closed, "ready"),
            ("s5", "E", SessionState::Error("x".into()), "ready"),
            ("s6", "F", SessionState::Active, "ready"),
            ("s7", "G", SessionState::Active, "ready"),
            ("s8", "H", SessionState::Active, "ready"),
            ("s9", "I", SessionState::Active, "ready"),
            ("s10", "J", SessionState::Active, "mystery"),
        ] {
            let idx = runs.add_session(id.into(), label.into());
            runs.sessions[idx].session_state = state;
            runs.sessions[idx].exec_state = exec.into();
            runs.sessions[idx].cwd = Some(if id == "s10" {
                "あ".repeat(30)
            } else {
                "/work".into()
            });
            runs.sessions[idx].has_unread = id == "s2";
        }
        runs.input_mode = InputMode::Switcher;
        runs.switcher_cursor = 1;
        runs.active_index = 0;
        let shown = draw_text(&mut runs);
        assert!(shown.contains("Runs") || shown.contains("RUNS"), "{shown}");

        let mut review = Model::new();
        mark(&mut review, SessionState::Active, "ready");
        let mut proposal = sample_proposal("echo hi", false);
        proposal.risk = "medium".into();
        proposal.destructive = true;
        proposal.escalates_privileges = true;
        proposal.touches_network = true;
        proposal.confidence = 0.42;
        review.set_proposal(proposal, "missing-owner".into());
        review.input_mode = InputMode::Review;
        review.review_error = Some("clipped".into());
        review.status_line = Some("review status".into());
        review.proposal_confirmed = true;
        let shown = draw_text(&mut review);
        assert!(shown.contains("MEDIUM") || shown.contains("medium") || shown.contains("Risk"), "{shown}");
        assert!(shown.contains("DESTRUCTIVE"), "{shown}");

        let mut bare = Model::new();
        bare.input_mode = InputMode::Review;
        let shown = draw_text(&mut bare);
        assert!(shown.contains("No proposal"), "{shown}");

        let mut high = Model::new();
        mark(&mut high, SessionState::Active, "ready");
        let mut proposal = sample_proposal("echo hi", true);
        proposal.risk = "severe".into();
        high.set_proposal(proposal, "s1".into());
        high.input_mode = InputMode::Review;
        high.proposal_confirmed = false;
        let shown = draw_text(&mut high);
        assert!(shown.contains("Confirmation required"), "{shown}");

        let mut raw = Model::new();
        mark(&mut raw, SessionState::Active, "ready");
        raw.input_mode = InputMode::RawPlay;
        let shown = draw_text(&mut raw);
        assert!(!shown.contains("Ask the AI") || shown.contains("CommandUI"), "{shown}");

        let mut tiny = Model::new();
        mark(&mut tiny, SessionState::Active, "ready");
        tiny.show_help = true;
        tiny.input_mode = InputMode::Switcher;
        let shown = draw_sized(&mut tiny, 4, 3);
        assert!(!shown.is_empty());
    }

    fn active_model(lines: Vec<String>) -> Model {
        let mut model = Model::new();
        mark(&mut model, SessionState::Active, "ready");
        model.sessions[0].terminal_lines = lines;
        model
    }

    #[test]
    fn long_lines_do_not_push_the_prompt_out_of_the_pane() {
        // 24 rows: status, pane (22 incl. borders), footer. Inner width is 78.
        let mut lines: Vec<String> = (0..30).map(|i| format!("old-{i}")).collect();
        // Four long lines, each wrapping to several rows, then the prompt.
        for i in 0..4 {
            lines.push(format!("{i}{}", "w".repeat(300)));
        }
        let mut model = active_model(lines);
        model.sessions[0].line_remainder = "PROMPT> ".into();
        let shown = draw_text(&mut model);
        assert!(shown.contains("PROMPT> "), "newest row must stay in view:\n{shown}");
        // Wrapped rows of the last long line are on screen too.
        assert!(shown.contains(&"w".repeat(78)), "{shown}");
        assert!(!shown.contains("old-0"), "{shown}");
    }

    #[test]
    fn scrolled_up_view_ends_on_the_anchored_line_and_the_footer_counts_below() {
        let lines: Vec<String> = (0..100).map(|i| format!("row-{i:03}")).collect();
        let mut model = active_model(lines);
        model.sessions[0].line_remainder = "open-line".into();
        model.sessions[0].scroll_offset = 40;
        let shown = draw_text(&mut model);
        assert!(shown.contains("row-059"), "{shown}");
        assert!(!shown.contains("row-060"), "{shown}");
        assert!(!shown.contains("open-line"), "{shown}");
        assert!(shown.contains("[40 below]"), "{shown}");
        assert!(!shown.contains("above"), "{shown}");
    }

    #[test]
    fn dropped_scrollback_is_marked_at_the_top_of_the_pane() {
        let mut model = active_model(vec!["kept".into()]);
        let none = draw_text(&mut model);
        assert!(!none.contains("dropped"), "{none}");
        model.sessions[0].lines_dropped = 1234;
        model.sessions[0].bytes_dropped = 99;
        let shown = draw_text(&mut model);
        assert!(shown.contains("[1234 lines, 99 bytes dropped]"), "{shown}");
        assert!(shown.contains("kept"), "{shown}");
        model.sessions[0].bytes_dropped = 0;
        assert!(draw_text(&mut model).contains("[1234 lines dropped]"));
    }

    #[test]
    fn rendered_lines_hold_no_erase_residue_or_color_parameters() {
        let mut model = active_model(vec![]);
        model.apply_event(commandui_runtime_core::events::RuntimeEvent::TerminalLine(
            commandui_runtime_core::events::TerminalLineEvent {
                id: "l".into(),
                session_id: "s1".into(),
                execution_id: None,
                kind: "stdout".into(),
                text: "ls\u{8} \u{8}s\n\u{1b}[01;34mdir\u{1b}[0m\n".into(),
                timestamp: "t".into(),
            },
        ));
        let shown = draw_text(&mut model);
        assert!(shown.contains("ls "), "{shown}");
        assert!(!shown.contains("ls s"), "erase pair must not leave residue:
{shown}");
        assert!(shown.contains("dir"), "{shown}");
        assert!(!shown.contains("[01;34m"), "{shown}");
        assert!(!shown.contains("[0m"), "{shown}");
    }

    #[test]
    fn wrap_rows_breaks_by_display_width_and_expands_tabs() {
        assert_eq!(wrap_rows("", 5), vec![String::new()]);
        assert_eq!(wrap_rows("abcdefg", 3), vec!["abc", "def", "g"]);
        // Fullwidth characters are two cells and never split across rows.
        assert_eq!(wrap_rows("\u{65e5}\u{65e5}\u{65e5}", 5), vec!["\u{65e5}\u{65e5}", "\u{65e5}"]);
        assert_eq!(wrap_rows("a\tb", 20), vec!["a       b"]);
        assert_eq!(wrap_rows("a\u{7}b", 20), vec!["ab"]);
        assert!(wrap_rows("abc", 0).is_empty());
    }

    #[test]
    fn page_lines_follow_visual_rows() {
        let mut s = crate::model::SessionModel::new("s".into(), "A".into());
        s.terminal_lines = (0..20).map(|_| "x".repeat(30)).collect();
        // 10-wide pane: each line is 3 rows, so 9 rows cover 3 lines.
        assert_eq!(page_lines(&s, 10, 9), 3);
        assert_eq!(page_lines(&s, 100, 9), 9);
        assert_eq!(page_lines(&s, 10, 0), 1);
    }

    #[test]
    fn a_fullwidth_command_is_clipped_by_cells_and_a_huge_scroll_keeps_the_gate_closed() {
        // 3 + 20 + 1 cells wide, 14 chars.
        let line = vec![format!("$ '{}'", "\u{65e5}".repeat(10))];
        assert_eq!(widest_line(&line), 24);
        assert!(line[0].chars().count() < 20);
        assert!(command_is_clipped(&line, 0, 0, 4, 20));
        assert!(!command_is_clipped(&line, 0, 4, 4, 20));
        assert!(command_is_clipped(&line, 70_000, 0, 4, 20));
        assert!(command_is_clipped(&line, 0, 70_000, 4, 80));
    }

    #[test]
    fn help_row_matches_the_keys_handle_shell_key_implements() {
        let mut model = active_model(vec![]);
        model.show_help = true;
        let shown = draw_text(&mut model);
        assert!(shown.contains("^] next"), "{shown}");
        assert!(shown.contains("^[ prev"), "{shown}");
        assert!(!shown.contains("Prev/next"), "{shown}");
    }

    #[test]
    fn status_bar_shows_zero_of_zero_with_no_sessions() {
        let mut model = Model::new();
        let shown = draw_text(&mut model);
        assert!(shown.contains("[0/0]"), "{shown}");
        assert!(!shown.contains("[1/0]"), "{shown}");
    }

    #[test]
    fn the_session_error_and_the_create_failure_are_readable() {
        let mut model = Model::new();
        mark(&mut model, SessionState::Error("pty open failed: no tty".into()), "ready");
        let shown = draw_text(&mut model);
        assert!(shown.contains("pty open failed: no tty"), "{shown}");

        let mut empty = Model::new();
        empty.create_error = Some("Could not start Session 1: spawn failed".into());
        let shown = draw_text(&mut empty);
        assert!(shown.contains("Press ^N"), "{shown}");
        assert!(shown.contains("Could not start Session 1: spawn failed"), "{shown}");
    }

    #[test]
    fn the_run_list_windows_around_the_cursor() {
        let mut model = Model::new();
        for i in 0..30 {
            let idx = model.add_session(format!("s{i}"), format!("Run-{i:02}"));
            model.sessions[idx].session_state = SessionState::Active;
            model.sessions[idx].exec_state = "ready".into();
        }
        model.input_mode = InputMode::Switcher;
        model.switcher_cursor = 0;
        let top = draw_text(&mut model);
        assert!(top.contains("Run-00"), "{top}");
        assert_eq!(model.switcher_start, 0);
        assert!(model.switcher_rows > 0 && model.switcher_rows < 30);

        model.switcher_cursor = 29;
        let bottom = draw_text(&mut model);
        assert!(bottom.contains("Run-29"), "cursor row must be drawn:\n{bottom}");
        assert!(!bottom.contains("Run-05"), "{bottom}");
        assert_eq!(model.switcher_start + model.switcher_rows, 30);

        assert_eq!(switcher_window(5, 4, 3), (2, 3));
        assert_eq!(switcher_window(5, 0, 3), (0, 3));
        assert_eq!(switcher_window(2, 1, 9), (0, 2));
        assert_eq!(switcher_window(5, 2, 0), (0, 0));
    }

    #[test]
    fn a_mock_proposal_says_the_model_call_failed() {
        let mut model = Model::new();
        mark(&mut model, SessionState::Active, "ready");
        model.set_proposal(sample_proposal("echo hi", false), "s1".into());
        model.input_mode = InputMode::Review;
        let shown = draw_text(&mut model);
        assert!(shown.contains("mock: model call failed"), "{shown}");
        let mut real = Model::new();
        mark(&mut real, SessionState::Active, "ready");
        let mut proposal = sample_proposal("echo hi", false);
        proposal.source = "ollama".into();
        real.set_proposal(proposal, "s1".into());
        real.input_mode = InputMode::Review;
        assert!(!draw_text(&mut real).contains("model call failed"));
    }

    #[test]
    fn review_helpers_cover_empty_and_saturated_edges() {
        assert_eq!(split_review_rows(0, 4), (0, 0));
        assert_eq!(scroll_u16(usize::MAX), u16::MAX);
        assert_eq!(shorten_cwd("/work"), "/work");
        let lines = visible_command_lines("");
        assert_eq!(lines.len(), 1);
        let quoted = visible_command_lines("a'b\t\\\u{2028}");
        assert!(quoted[0].contains("\\'"), "{quoted:?}");
        assert!(quoted[0].contains("\\t"), "{quoted:?}");
        assert!(quoted[0].contains("\\\\"), "{quoted:?}");
        assert!(command_is_clipped(&[], 0, 0, 1, 1) == false);

        let frame = Rect::new(2, 2, 4, 3);
        let host = Rect::new(0, 0, 1, 1);
        let overlay = clamped_overlay(frame, host, 10, 10);
        assert!(overlay.width <= frame.width);
        assert!(overlay.height <= frame.height);
    }

    #[test]
    fn review_meta_puts_flags_first_and_wraps_instead_of_clipping() {
        let mut p = sample_proposal("echo caf\u{e9}", true);
        p.destructive = true;
        p.touches_network = true;
        p.escalates_privileges = true;
        p.cwd = Some("/very/long/directory/that/names/where/it/runs".into());
        let mut model = Model::new();
        model.set_proposal(p, "s1".into());
        let lines = review_meta_lines(&model, 30);
        let text: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect();
        // Non-ASCII leads the flags and is on the first row.
        assert!(text[0].starts_with("Flags: non-ASCII"), "{text:?}");
        // No row is wider than the pane, and every flag survives the wrap.
        // The risk badge row is short and left as is.
        assert!(
            text.iter()
                .filter(|r| !r.starts_with("Risk:"))
                .all(|row| row.chars().count() <= 30),
            "{text:?}"
        );
        let joined = text.join(" ");
        for flag in ["DESTRUCTIVE", "PRIVILEGE_ESCALATION", "NETWORK_ACCESS"] {
            assert!(joined.replace("  ", " ").contains(flag) || text.iter().any(|r| r.contains(flag)), "{flag}: {text:?}");
        }
        // The confirmation notice and the error row precede risk and directory.
        let conf = text.iter().position(|r| r.starts_with("Confirmation")).unwrap();
        let risk = text.iter().position(|r| r.starts_with("Risk:")).unwrap();
        let runs = text.iter().position(|r| r.starts_with("Runs in:")).unwrap();
        assert!(conf < risk && risk < runs, "{text:?}");
        // The directory is wrapped whole, not cut to its first characters.
        assert!(text.join("").contains("where/it/runs"), "{text:?}");
    }

    #[test]
    fn a_short_pane_keeps_the_flags_and_the_error_and_drops_the_explanation() {
        let mut p = sample_proposal("echo caf\u{e9}", false);
        p.destructive = true;
        let mut model = Model::new();
        model.set_proposal(p, "s1".into());
        model.review_error = Some("Directory changed".into());
        let lines = review_meta_lines(&model, 60);
        let (command_rows, meta_rows) = split_review_rows(4, lines.len());
        assert!(command_rows >= 1);
        let shown: Vec<String> = lines
            .iter()
            .take(meta_rows)
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect();
        assert!(shown.iter().any(|r| r.contains("non-ASCII")), "{shown:?}");
        assert!(shown.iter().any(|r| r.contains("Directory changed")), "{shown:?}");
        assert!(!shown.iter().any(|r| r.starts_with("Explanation")), "{shown:?}");
    }

    #[test]
    fn the_confirm_prompt_replaces_the_footer_hints() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.sessions[0].session_state = crate::model::SessionState::Active;
        model.sessions[0].exec_state = "running".into();
        model.ask_confirm(crate::model::PendingConfirm::CloseSession("s1".into()));
        let shown = draw_text(&mut model);
        assert!(shown.contains("Close session with a running command? y/n"), "{shown}");
    }
}
