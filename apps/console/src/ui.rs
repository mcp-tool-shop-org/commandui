//! Console UI renderer — multi-session aware.
//!
//! Renders the active session's state. Status bar shows session indicator.

use crate::model::{InputMode, Model, SessionState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, model: &Model) {
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

fn render_review_layout(frame: &mut Frame, model: &Model) {
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
    render_review_footer(frame, chunks[3]);
}

fn render_switcher_layout(frame: &mut Frame, model: &Model) {
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

    let (state_label, state_color) = match active.map(|s| &s.session_state) {
        Some(SessionState::Booting) => ("BOOT", Color::Yellow),
        Some(SessionState::Active) => ("READY", Color::Green),
        Some(SessionState::Closed) => ("DONE", Color::Red),
        Some(SessionState::Error(_)) => ("ERROR", Color::Red),
        None => ("NO SESSION", Color::Red),
    };

    let exec_color = match active.map(|s| s.exec_state.as_str()) {
        Some("ready") => Color::Green,
        Some("running") => Color::Cyan,
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
    let session_indicator = format!(
        "[{}/{}]",
        model.active_index + 1,
        model.session_count()
    );

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
        Span::raw(" "),
        Span::styled(
            state_label,
            Style::default().fg(state_color).add_modifier(Modifier::BOLD),
        ),
    ];

    if active.map_or(false, |s| s.session_state == SessionState::Active) {
        let exec_display = match active.map(|s| s.exec_state.as_str()) {
            Some("ready") => "idle",
            Some("interrupting") => "stopping",
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

    let inner_height = block.inner(area).height as usize;

    let visible_lines: Vec<Line> = if let Some(session) = active {
        let total = session.terminal_lines.len();
        let visible_end = if session.scroll_offset > 0 {
            total.saturating_sub(session.scroll_offset)
        } else {
            total
        };
        let visible_start = visible_end.saturating_sub(inner_height);

        if total == 0 && !is_ready {
            vec![Line::from(Span::styled(
                "Starting shell...",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            session.terminal_lines[visible_start..visible_end]
                .iter()
                .map(|s| Line::from(s.as_str()))
                .collect()
        }
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  CommandUI Console — terminal shell with sidecar AI",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled("    ^T  Ask the AI           ^G  Raw Play (fullscreen)", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("    ^S  Switch runs           ^N  New session", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("    ^H  Help                  ^Q  Quit", Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from(Span::styled(
                "  Press ^N to start a session.",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let paragraph = Paragraph::new(visible_lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
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

    if model.can_accept_input() {
        spans.push(Span::raw("  ^T Ask  ^G Raw Play  ^S Runs  ^H Help  ^Q Quit"));
    } else {
        spans.push(Span::raw("  ^N New  ^Q Quit"));
    }

    if let Some(s) = model.active_session() {
        if s.scroll_offset > 0 {
            spans.push(Span::styled(
                format!("  [{} above]", s.scroll_offset),
                Style::default().fg(Color::Yellow),
            ));
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
        .title(if model.planner_busy {
            " Ask (generating...) "
        } else {
            " Ask — describe what you want "
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
        let before = &text[..cursor];
        let cursor_char = text[cursor..].chars().next().unwrap_or(' ');
        let after_start = cursor + cursor_char.len_utf8().min(text.len() - cursor);
        let after = if after_start <= text.len() { &text[after_start..] } else { "" };

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

fn render_review_panel(frame: &mut Frame, area: Rect, model: &Model) {
    // Show which session this proposal targets in the title
    let title = if let Some(ref owner_id) = model.proposal_session_id {
        let owner_label = model
            .sessions
            .iter()
            .find(|s| s.id == *owner_id)
            .map(|s| s.label.as_str())
            .unwrap_or("?");
        format!(" Review Proposal → {owner_label} ")
    } else {
        " Review Proposal ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(title);

    let content = if let Some(ref proposal) = model.current_proposal {
        let risk_color = match proposal.risk.as_str() {
            "low" => Color::Green,
            "medium" => Color::Yellow,
            "high" => Color::Red,
            _ => Color::White,
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Command: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    &proposal.command,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Risk:    ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" {} ", proposal.risk.to_uppercase()),
                    Style::default().fg(Color::Black).bg(risk_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  confidence: {:.0}%", proposal.confidence * 100.0)),
                Span::raw(format!("  source: {}", proposal.source)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Explanation: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&proposal.explanation),
            ]),
        ];

        let mut safety_flags = vec![];
        if proposal.destructive { safety_flags.push("DESTRUCTIVE"); }
        if proposal.escalates_privileges { safety_flags.push("PRIVILEGE_ESCALATION"); }
        if proposal.touches_network { safety_flags.push("NETWORK_ACCESS"); }

        if !safety_flags.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Flags: ", Style::default().fg(Color::Red)),
                Span::styled(safety_flags.join(", "), Style::default().fg(Color::Red)),
            ]));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "No proposal to review.",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_review_footer(frame: &mut Frame, area: Rect) {
    let spans = vec![
        Span::styled(
            " REVIEW ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Enter approve  Esc cancel  ^Q Quit"),
    ];

    let footer = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

// ---- Run selector overlay ----

fn render_run_selector_overlay(frame: &mut Frame, area: Rect, model: &Model) {
    // Center the overlay in the terminal pane area
    let list_height = (model.session_count() as u16 + 2).min(area.height); // +2 for border
    let list_width = area.width.min(60).max(30);

    let x = area.x + (area.width.saturating_sub(list_width)) / 2;
    let y = area.y + (area.height.saturating_sub(list_height)) / 2;

    let overlay_area = Rect::new(x, y, list_width, list_height);

    // Build the list
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Runs ");

    let mut lines: Vec<Line> = Vec::new();

    for (idx, session) in model.sessions.iter().enumerate() {
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

        // Number hint (1-9)
        let num = if idx < 9 {
            format!("{}", idx + 1)
        } else {
            " ".to_string()
        };

        // CWD (shortened)
        let cwd_short = session.cwd.as_deref().unwrap_or("...");
        let cwd_display = if cwd_short.len() > 25 {
            format!("...{}", &cwd_short[cwd_short.len() - 22..])
        } else {
            cwd_short.to_string()
        };

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
    let bg = Paragraph::new(vec![Line::from(""); list_height as usize])
        .style(Style::default().bg(Color::Black));
    frame.render_widget(bg, overlay_area);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, overlay_area);
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
    let help_width = area.width.min(56).max(30);
    let help_height = area.height.min(18).max(6);

    let x = area.x + (area.width.saturating_sub(help_width)) / 2;
    let y = area.y + (area.height.saturating_sub(help_height)) / 2;

    let overlay_area = Rect::new(x, y, help_width, help_height);

    // Background
    let bg = Paragraph::new(vec![Line::from(""); help_height as usize])
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
            Span::raw(" ^]/[  Prev/next        "),
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
        Line::from(Span::styled(" Esc or ^H to close", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, overlay_area);
}
