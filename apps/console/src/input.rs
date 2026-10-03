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
    if let Some(encoded) = encode_control_char(key) {
        return encoded;
    }
    if let Some(seq) = encode_special(key.code, key.modifiers) {
        return seq;
    }
    if let KeyCode::Char(c) = key.code {
        // Unknown Ctrl chords are not cast to a byte. Alt is ESC plus the key.
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return String::new();
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            return format!("\u{1b}{c}");
        }
        return c.to_string();
    }
    String::new()
}

/// Ctrl+letter (either case) is the control byte. Alt adds an ESC prefix.
/// Non-ASCII is not truncated with `as u8`.
fn encode_control_char(key: KeyEvent) -> Option<String> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    let byte = control_byte(c)?;
    let mut out = String::new();
    if key.modifiers.contains(KeyModifiers::ALT) {
        out.push('\u{1b}');
    }
    out.push(byte as char);
    Some(out)
}

fn control_byte(c: char) -> Option<u8> {
    if !c.is_ascii() {
        return None;
    }
    let upper = c.to_ascii_uppercase() as u8;
    match upper {
        b'A'..=b'Z' | b'@' | b'[' | b'\\' | b']' | b'^' | b'_' => Some(upper & 0x1f),
        b' ' => Some(0),
        _ => None,
    }
}

/// xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl. 1 means unmodified.
fn modifier_param(mods: KeyModifiers) -> u8 {
    let mut param = 1u8;
    if mods.contains(KeyModifiers::SHIFT) {
        param += 1;
    }
    if mods.contains(KeyModifiers::ALT) {
        param += 2;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        param += 4;
    }
    param
}

fn encode_special(code: KeyCode, mods: KeyModifiers) -> Option<String> {
    let param = modifier_param(mods);
    let seq = match code {
        KeyCode::Up => arrow('A', param),
        KeyCode::Down => arrow('B', param),
        KeyCode::Right => arrow('C', param),
        KeyCode::Left => arrow('D', param),
        KeyCode::Home => {
            if param == 1 {
                "\x1b[H".to_string()
            } else {
                format!("\x1b[1;{param}H")
            }
        }
        KeyCode::End => {
            if param == 1 {
                "\x1b[F".to_string()
            } else {
                format!("\x1b[1;{param}F")
            }
        }
        KeyCode::Insert => tilde(2, param),
        KeyCode::Delete => tilde(3, param),
        KeyCode::PageUp => tilde(5, param),
        KeyCode::PageDown => tilde(6, param),
        KeyCode::F(n) => function_key(n, param)?,
        KeyCode::Enter => alt_prefix(mods, "\r"),
        KeyCode::Backspace => alt_prefix(mods, "\x7f"),
        KeyCode::Tab => alt_prefix(mods, "\t"),
        KeyCode::Esc => alt_prefix(mods, "\x1b"),
        _ => return None,
    };
    Some(seq)
}

fn arrow(letter: char, param: u8) -> String {
    if param == 1 {
        format!("\x1b[{letter}")
    } else {
        format!("\x1b[1;{param}{letter}")
    }
}

fn tilde(code: u8, param: u8) -> String {
    if param == 1 {
        format!("\x1b[{code}~")
    } else {
        format!("\x1b[{code};{param}~")
    }
}

fn alt_prefix(mods: KeyModifiers, bytes: &str) -> String {
    if mods.contains(KeyModifiers::ALT) {
        format!("\x1b{bytes}")
    } else {
        bytes.to_string()
    }
}

fn function_key(n: u8, param: u8) -> Option<String> {
    let unmodified = match n {
        1 => "\x1bOP",
        2 => "\x1bOQ",
        3 => "\x1bOR",
        4 => "\x1bOS",
        5 => "\x1b[15~",
        6 => "\x1b[17~",
        7 => "\x1b[18~",
        8 => "\x1b[19~",
        9 => "\x1b[20~",
        10 => "\x1b[21~",
        11 => "\x1b[23~",
        12 => "\x1b[24~",
        _ => return None,
    };
    if param == 1 {
        return Some(unmodified.to_string());
    }
    let modified = match n {
        1 => format!("\x1b[1;{param}P"),
        2 => format!("\x1b[1;{param}Q"),
        3 => format!("\x1b[1;{param}R"),
        4 => format!("\x1b[1;{param}S"),
        5 => format!("\x1b[15;{param}~"),
        6 => format!("\x1b[17;{param}~"),
        7 => format!("\x1b[18;{param}~"),
        8 => format!("\x1b[19;{param}~"),
        9 => format!("\x1b[20;{param}~"),
        10 => format!("\x1b[21;{param}~"),
        11 => format!("\x1b[23;{param}~"),
        12 => format!("\x1b[24;{param}~"),
        _ => return None,
    };
    Some(modified)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn alt_b_is_esc_then_b() {
        let bytes = key_to_bytes(press(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(bytes, "\u{1b}b");
    }

    #[test]
    fn ctrl_letter_is_the_control_byte() {
        let lower = key_to_bytes(press(KeyCode::Char('d'), KeyModifiers::CONTROL));
        let upper = key_to_bytes(press(KeyCode::Char('D'), KeyModifiers::CONTROL));
        assert_eq!(lower, "\u{4}");
        assert_eq!(upper, "\u{4}");
    }

    #[test]
    fn shift_up_is_a_modified_cursor_sequence() {
        let bytes = key_to_bytes(press(KeyCode::Up, KeyModifiers::SHIFT));
        assert_eq!(bytes, "\u{1b}[1;2A");
        let plain = key_to_bytes(press(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(plain, "\u{1b}[A");
    }
}
