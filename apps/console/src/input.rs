//! Console input routing — mode-aware, session-targeted key ownership.
//!
//! Session targeting law: all input targets the ACTIVE session only.
//! Session switching is Console-owned and changes which session is active.
//!
//! SHELL mode:
//!   - Ctrl+Q       → quit
//!   - Ctrl+C       → interrupt active session (or ETX when idle)
//!   - Ctrl+R       → resync active session
//!   - Ctrl+T       → switch to ASK mode
//!   - Ctrl+N       → create new session
//!   - Ctrl+W       → close active session
//!   - Ctrl+Tab / Ctrl+] → next session
//!   - Ctrl+[ / Shift+Ctrl+Tab → previous session
//!   - Shift+PgUp   → scroll active session up
//!   - Shift+PgDn   → scroll active session down
//!   - All else     → forward to active session shell
//!
//! ASK mode:
//!   - Ctrl+Q       → quit
//!   - Escape/Ctrl+T → back to SHELL
//!   - Enter         → submit intent (targets active session)
//!   - Backspace/Left/Right/Ctrl+U → edit composer
//!   - Printable    → insert into composer
//!
//! REVIEW mode:
//!   - Ctrl+Q       → quit
//!   - Enter/y      → approve (execute on active session)
//!   - Escape/n     → cancel
//!
//! Fullscreen child policy: NOT SUPPORTED YET.

use crate::model::{InputMode, Model};
use commandui_runtime_core::services::terminal_service::TerminalService;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub enum InputAction {
    Forwarded,
    Interrupted,
    Resynced,
    Scrolled,
    ModeSwitched,
    SubmitIntent(String),
    /// Approve proposal — carries (command, owning_session_id).
    ApproveProposal(String, String),
    CancelProposal,
    NextSession,
    PrevSession,
    CreateSession,
    CloseSession,
    /// Enter raw play mode — game owns the terminal.
    EnterRawPlay,
    /// Exit raw play mode — Console resumes.
    ExitRawPlay,
    Quit,
    Ignored,
}

pub fn handle_key(
    key: KeyEvent,
    model: &mut Model,
    terminal_service: &TerminalService,
) -> InputAction {
    // Ctrl+Q — quit (always)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return InputAction::Quit;
    }

    match model.input_mode {
        InputMode::Shell => handle_shell_key(key, model, terminal_service),
        InputMode::Ask => handle_ask_key(key, model),
        InputMode::Review => handle_review_key(key, model),
        InputMode::Switcher => handle_switcher_key(key, model),
        InputMode::RawPlay => handle_raw_play_key(key, model, terminal_service),
    }
}

fn handle_shell_key(
    key: KeyEvent,
    model: &mut Model,
    terminal_service: &TerminalService,
) -> InputAction {
    // Help overlay — Ctrl+H toggles, Esc dismisses
    if model.show_help {
        match key.code {
            KeyCode::Esc => {
                model.show_help = false;
                return InputAction::Ignored;
            }
            _ if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('h') =>
            {
                model.show_help = false;
                return InputAction::Ignored;
            }
            _ => return InputAction::Ignored, // Swallow all keys while help is open
        }
    }

    // Scrollback — Shift+PageUp/Down (targets active session)
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::PageUp => {
                let page = model.pane_rows.max(1) as usize;
                if let Some(s) = model.active_session_mut() {
                    s.scroll_up(page);
                }
                return InputAction::Scrolled;
            }
            KeyCode::PageDown => {
                let page = model.pane_rows.max(1) as usize;
                if let Some(s) = model.active_session_mut() {
                    s.scroll_down(page);
                }
                return InputAction::Scrolled;
            }
            _ => {}
        }
    }

    // Session management
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            // Ctrl+H — toggle help overlay
            KeyCode::Char('h') => {
                model.show_help = true;
                return InputAction::Ignored;
            }
            // Ctrl+G — enter raw play mode (game mode)
            KeyCode::Char('g') => {
                if model.can_accept_input() {
                    model.input_mode = InputMode::RawPlay;
                    return InputAction::EnterRawPlay;
                }
                return InputAction::Ignored;
            }
            // Ctrl+S — open run selector
            KeyCode::Char('s') => {
                model.open_switcher();
                return InputAction::ModeSwitched;
            }
            // Ctrl+T — switch to ASK mode
            KeyCode::Char('t') => {
                if model.can_accept_input() {
                    model.input_mode = InputMode::Ask;
                    model.planner_error = None;
                    return InputAction::ModeSwitched;
                }
                return InputAction::Ignored;
            }
            // Ctrl+N — create new session
            KeyCode::Char('n') => {
                return InputAction::CreateSession;
            }
            // Ctrl+W — close active session
            KeyCode::Char('w') => {
                if model.session_count() > 1 {
                    return InputAction::CloseSession;
                }
                return InputAction::Ignored; // Don't close last session
            }
            // Ctrl+] — next session
            KeyCode::Char(']') => {
                if model.session_count() > 1 {
                    model.next_session();
                    return InputAction::NextSession;
                }
                return InputAction::Ignored;
            }
            // Ctrl+[ — previous session
            KeyCode::Char('[') => {
                if model.session_count() > 1 {
                    model.prev_session();
                    return InputAction::PrevSession;
                }
                return InputAction::Ignored;
            }
            _ => {}
        }
    }

    // Everything below requires an active session
    let session_id = match model.active_session_id() {
        Some(id) => id.to_string(),
        None => return InputAction::Ignored,
    };

    if !model.can_accept_input() {
        return InputAction::Ignored;
    }

    // Ctrl+C — interrupt or forward ETX
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        let exec_state = model.active_session().map(|s| s.exec_state.as_str());
        if exec_state == Some("running") || exec_state == Some("interrupting") {
            let _ = terminal_service.interrupt(&session_id);
            return InputAction::Interrupted;
        } else {
            let _ = terminal_service.write(&session_id, "\x03");
            return InputAction::Forwarded;
        }
    }

    // Ctrl+R — resync
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        let _ = terminal_service.resync(&session_id);
        return InputAction::Resynced;
    }

    // Snap to bottom on typing
    if let Some(s) = model.active_session_mut() {
        if s.scroll_offset > 0 {
            s.scroll_to_bottom();
        }
    }

    // Forward key to active session's shell
    let data = key_to_bytes(key);
    if !data.is_empty() {
        let _ = terminal_service.write(&session_id, &data);
        return InputAction::Forwarded;
    }

    InputAction::Ignored
}

fn handle_ask_key(key: KeyEvent, model: &mut Model) -> InputAction {
    if model.planner_busy {
        return InputAction::Ignored;
    }

    match key.code {
        KeyCode::Esc => {
            model.input_mode = InputMode::Shell;
            InputAction::ModeSwitched
        }
        _ if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') => {
            model.input_mode = InputMode::Shell;
            InputAction::ModeSwitched
        }
        KeyCode::Enter => {
            let text = model.composer_text.trim().to_string();
            if text.is_empty() {
                return InputAction::Ignored;
            }
            model.planner_busy = true;
            model.planner_error = None;
            InputAction::SubmitIntent(text)
        }
        _ if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') => {
            model.composer_clear();
            InputAction::Ignored
        }
        KeyCode::Backspace => {
            model.composer_backspace();
            InputAction::Ignored
        }
        KeyCode::Left => {
            model.composer_left();
            InputAction::Ignored
        }
        KeyCode::Right => {
            model.composer_right();
            InputAction::Ignored
        }
        KeyCode::Char(c) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                model.composer_insert(c);
            }
            InputAction::Ignored
        }
        _ => InputAction::Ignored,
    }
}

fn handle_review_key(key: KeyEvent, model: &mut Model) -> InputAction {
    match key.code {
        KeyCode::Enter | KeyCode::Char('y') => {
            if let (Some(ref proposal), Some(ref session_id)) =
                (&model.current_proposal, &model.proposal_session_id)
            {
                let command = proposal.command.clone();
                let target_session = session_id.clone();
                model.input_mode = InputMode::Shell;
                model.clear_proposal();
                model.composer_clear();
                InputAction::ApproveProposal(command, target_session)
            } else {
                InputAction::Ignored
            }
        }
        KeyCode::Esc | KeyCode::Char('n') => {
            model.input_mode = InputMode::Ask;
            model.clear_proposal();
            InputAction::CancelProposal
        }
        _ => InputAction::Ignored,
    }
}

/// Raw play mode — nearly all keys forwarded to the game.
/// Only the escape chord (Ctrl+\) returns to Console.
/// Ctrl+Q also exits raw mode first, then quits.
fn handle_raw_play_key(
    key: KeyEvent,
    model: &mut Model,
    terminal_service: &TerminalService,
) -> InputAction {
    // Escape chord: Ctrl+\ — exit raw play mode, return to Console
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('\\') {
        model.input_mode = InputMode::Shell;
        return InputAction::ExitRawPlay;
    }

    // Ctrl+Q during raw play — exit raw mode first, then quit
    // (handle_key already caught Ctrl+Q for non-raw modes, but raw mode
    //  needs special handling to restore the terminal before quitting)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        model.input_mode = InputMode::Shell;
        model.should_quit = true;
        return InputAction::ExitRawPlay;
    }

    // If the active session is gone, force exit raw mode
    if !model.active_session_alive() {
        model.input_mode = InputMode::Shell;
        return InputAction::ExitRawPlay;
    }

    // Everything else goes to the game — no Console interception
    let session_id = match model.active_session_id() {
        Some(id) => id.to_string(),
        None => {
            model.input_mode = InputMode::Shell;
            return InputAction::ExitRawPlay;
        }
    };

    let data = key_to_bytes(key);
    if !data.is_empty() {
        let _ = terminal_service.write(&session_id, &data);
        return InputAction::Forwarded;
    }

    InputAction::Ignored
}

fn handle_switcher_key(key: KeyEvent, model: &mut Model) -> InputAction {
    let count = model.session_count();
    if count == 0 {
        model.close_switcher();
        return InputAction::ModeSwitched;
    }

    match key.code {
        // Navigate
        KeyCode::Up | KeyCode::Char('k') => {
            model.switcher_cursor = if model.switcher_cursor == 0 {
                count - 1
            } else {
                model.switcher_cursor - 1
            };
            InputAction::Ignored
        }
        KeyCode::Down | KeyCode::Char('j') => {
            model.switcher_cursor = (model.switcher_cursor + 1) % count;
            InputAction::Ignored
        }

        // Confirm selection
        KeyCode::Enter => {
            model.confirm_switcher();
            InputAction::NextSession // signals app to resize PTY
        }

        // Close without switching
        KeyCode::Esc => {
            model.close_switcher();
            InputAction::ModeSwitched
        }
        _ if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') => {
            model.close_switcher();
            InputAction::ModeSwitched
        }

        // Create new session
        _ if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') => {
            model.close_switcher();
            InputAction::CreateSession
        }

        // Close selected session
        _ if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('w') => {
            if count > 1 {
                // Close the session at switcher_cursor, not active
                model.close_switcher();
                // Switch to the cursor target first so close_active_session closes it
                model.switch_to(model.switcher_cursor);
                return InputAction::CloseSession;
            }
            InputAction::Ignored
        }

        // Number keys 1-9 for direct jump
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let idx = (c as usize) - ('1' as usize);
            if idx < count {
                model.switcher_cursor = idx;
                model.confirm_switcher();
                InputAction::NextSession
            } else {
                InputAction::Ignored
            }
        }

        _ => InputAction::Ignored,
    }
}

fn key_to_bytes(key: KeyEvent) -> String {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let ctrl_byte = (c as u8).wrapping_sub(b'a').wrapping_add(1);
            if ctrl_byte <= 26 {
                return String::from(ctrl_byte as char);
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "\r".to_string(),
        KeyCode::Backspace => "\x7f".to_string(),
        KeyCode::Tab => "\t".to_string(),
        KeyCode::Esc => "\x1b".to_string(),
        KeyCode::Up => "\x1b[A".to_string(),
        KeyCode::Down => "\x1b[B".to_string(),
        KeyCode::Right => "\x1b[C".to_string(),
        KeyCode::Left => "\x1b[D".to_string(),
        KeyCode::Home => "\x1b[H".to_string(),
        KeyCode::End => "\x1b[F".to_string(),
        KeyCode::Delete => "\x1b[3~".to_string(),
        KeyCode::PageUp => "\x1b[5~".to_string(),
        KeyCode::PageDown => "\x1b[6~".to_string(),
        KeyCode::Insert => "\x1b[2~".to_string(),
        KeyCode::F(n) => match n {
            1 => "\x1bOP".to_string(),
            2 => "\x1bOQ".to_string(),
            3 => "\x1bOR".to_string(),
            4 => "\x1bOS".to_string(),
            5 => "\x1b[15~".to_string(),
            6 => "\x1b[17~".to_string(),
            7 => "\x1b[18~".to_string(),
            8 => "\x1b[19~".to_string(),
            9 => "\x1b[20~".to_string(),
            10 => "\x1b[21~".to_string(),
            11 => "\x1b[23~".to_string(),
            12 => "\x1b[24~".to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
