//! Console input routing — mode-aware, session-targeted key ownership.
//!
//! Session targeting law: all input targets the ACTIVE session only.
//! Session switching is Console-owned and changes which session is active.
//!
//! SHELL mode:
//!   - Ctrl+Q       → quit; asks y/n first while any session has a running command
//!   - Ctrl+C       → interrupt active session (or ETX when idle)
//!   - Ctrl+R       → resync active session
//!   - Ctrl+T       → switch to ASK mode
//!   - Ctrl+G       → enter Raw Play
//!   - Ctrl+S       → open the run selector
//!   - F1 / Ctrl+/  → help overlay (Esc or F1 closes it)
//!   - Ctrl+N       → create new session
//!   - Ctrl+W       → close active session (the last one too); asks y/n first
//!     when it has a running command (only a bare y proceeds)
//!   - Ctrl+] / Ctrl+5 → next session (Unix crossterm 0.28 reports Ctrl+] as Ctrl+5)
//!   - Ctrl+[       → previous session (Windows reports the bracket)
//!   - Esc          → previous session on Unix only, where Ctrl+[ arrives as Esc,
//!     and only with more than one session; otherwise the shell receives ESC
//!   - Shift+PgUp / Shift+PgDn → scroll active session
//!   - All else     → forward to active session shell
//!
//! ASK mode:
//!   - Ctrl+Q       → quit
//!   - Escape/Ctrl+T → back to SHELL; while a proposal is generating, the same
//!     keys abort the request and its result is dropped
//!   - Enter         → submit intent (targets active session)
//!   - Backspace/Left/Right/Ctrl+U → edit composer
//!   - Printable    → insert into composer
//!
//! REVIEW mode (session switching is refused here and in Ask):
//!   - Ctrl+Q       → quit
//!   - Up/Down/PgUp/PgDn/Left/Right (j/k) → scroll the quoted command
//!   - c            → confirm, when the proposal requires it
//!   - Enter/y      → approve (executes on the proposal's session); refused while
//!     the command is clipped or confirmation is pending
//!   - Escape/n     → cancel and return to ASK
//!
//! RUNS (run selector): Up/Down/j/k, Enter, 1-9 (rows in view), Ctrl+N, Ctrl+W,
//! Esc or Ctrl+S to close.
//!
//! RAW PLAY: every key is forwarded to the PTY except Ctrl+\ (exit) and Ctrl+Q
//! (exit, then ask y/n to quit). Mouse, focus, and bracketed paste are forwarded in the
//! sequences the child enabled.

use crate::model::{InputMode, Model, PendingConfirm, SessionState};
use crate::ui::{
    command_is_clipped, page_lines, review_window, visible_command_lines, widest_line,
};
use commandui_runtime_core::services::terminal_service::TerminalService;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Whether a bare Esc is the Unix encoding of Ctrl+[ (previous session).
const ESC_IS_PREV_SESSION: bool = cfg!(unix);

#[derive(Debug, PartialEq)]
pub enum InputAction {
    Forwarded,
    Interrupted,
    Resynced,
    Scrolled,
    ModeSwitched,
    SubmitIntent(String),
    /// Approve proposal — carries (command, owning_session_id, proposal_id).
    /// The proposal stays in Review until execute returns Ok.
    ApproveProposal(String, String, String),
    CancelProposal,
    NextSession,
    PrevSession,
    CreateSession,
    CloseSession,
    /// Close the session with this id (run selector), leaving the active session alone.
    CloseSessionAt(String),
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
    // A destructive chord is waiting for y/n. Only a bare `y` proceeds; every
    // other key cancels it and is not passed on.
    if let Some(pending) = model.pending_confirm.take() {
        model.status_line = None;
        let yes = key.code == KeyCode::Char('y') && key.modifiers.is_empty();
        return match (yes, pending) {
            (true, PendingConfirm::CloseSession(id)) => InputAction::CloseSessionAt(id),
            (true, PendingConfirm::Quit) => InputAction::Quit,
            (false, _) => InputAction::Ignored,
        };
    }

    // Ctrl+Q — quit. Asks first while any session has a running command. Raw
    // Play handles its own Ctrl+Q: the child may use the chord.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('q')
        && model.input_mode != InputMode::RawPlay
    {
        if model.any_session_running() {
            if model.input_mode == InputMode::Switcher {
                model.close_switcher();
            }
            model.ask_confirm(PendingConfirm::Quit);
            return InputAction::Ignored;
        }
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

/// F1, or Ctrl+/ (Unix crossterm reports the 0x1f byte as Ctrl+7).
fn is_help_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::F(1)
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('/') | KeyCode::Char('7')))
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
            _ if is_help_key(&key) => {
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
                // A page is the logical lines that fill pane_rows visual rows.
                let page = scroll_page(model);
                if let Some(s) = model.active_session_mut() {
                    s.scroll_up(page);
                }
                return InputAction::Scrolled;
            }
            KeyCode::PageDown => {
                let page = scroll_page(model);
                if let Some(s) = model.active_session_mut() {
                    s.scroll_down(page);
                }
                return InputAction::Scrolled;
            }
            _ => {}
        }
    }

    // Help: F1 or Ctrl+/. Not Ctrl+H: terminals whose Backspace sends 0x08
    // report it as Ctrl+H, and Backspace must erase.
    if is_help_key(&key) {
        model.show_help = true;
        return InputAction::Ignored;
    }

    // Session management
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
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
            // Ctrl+W — close active session. The last one can close; the
            // empty welcome state is a supported state.
            KeyCode::Char('w') => {
                let Some(id) = model.active_session_id().map(str::to_string) else {
                    return InputAction::Ignored;
                };
                // The chord is also readline's word-rubout: never kill a running
                // command (an ssh session, a build) without asking.
                if model.session_is_running(&id) {
                    model.ask_confirm(PendingConfirm::CloseSession(id));
                    return InputAction::Ignored;
                }
                return InputAction::CloseSession;
            }
            // Ctrl+] — next session. Unix crossterm 0.28 emits this byte as Ctrl+5.
            KeyCode::Char(']') | KeyCode::Char('5') => {
                if model.session_count() > 1 {
                    model.next_session();
                    return InputAction::NextSession;
                }
                return InputAction::Ignored;
            }
            // Ctrl+[ — previous session. Windows reports the bracket; Unix reports Esc below.
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

    // Esc is Ctrl+[ on Unix crossterm 0.28, so there it is previous-session.
    // Where it is not (Windows reports Ctrl+[ itself), or with one session,
    // Esc falls through and the shell receives ESC. Help-open Esc is handled above.
    if ESC_IS_PREV_SESSION && key.code == KeyCode::Esc && model.session_count() > 1 {
        model.prev_session();
        return InputAction::PrevSession;
    }

    // Everything below requires an active session
    let session_id = match model.active_session_id() {
        Some(id) => id.to_string(),
        None => return InputAction::Ignored,
    };

    // A session still booting takes only Ctrl+R (resync), so a bootstrap that
    // never completes is not a dead pane. Typed text and Ctrl+C would interleave
    // with the bootstrap echo, and an Enter in Booting is not tracked as a run.
    let booting = model
        .active_session()
        .is_some_and(|s| s.session_state == SessionState::Booting);
    let is_resync_key = key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('r');
    if !model.can_accept_input() && !(booting && is_resync_key) {
        return InputAction::Ignored;
    }

    // Ctrl+C — interrupt or forward ETX
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        let exec_state = model.active_session().map(|s| s.exec_state.as_str());
        if exec_state == Some("running") || exec_state == Some("interrupting") {
            let result = terminal_service.interrupt(&session_id);
            model.surface_session_result(result);
            return if model.status_line.is_none() {
                InputAction::Interrupted
            } else {
                InputAction::Ignored
            };
        } else {
            let result = terminal_service.write(&session_id, "\x03");
            model.surface_session_result(result);
            return if model.status_line.is_none() {
                InputAction::Forwarded
            } else {
                InputAction::Ignored
            };
        }
    }

    // Ctrl+R — resync
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        let result = terminal_service.resync(&session_id);
        model.surface_session_result(result);
        return if model.status_line.is_none() {
            InputAction::Resynced
        } else {
            InputAction::Ignored
        };
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
        let result = terminal_service.write(&session_id, &data);
        model.surface_session_result(result);
        return if model.status_line.is_none() {
            InputAction::Forwarded
        } else {
            InputAction::Ignored
        };
    }

    InputAction::Ignored
}

fn scroll_page(model: &Model) -> usize {
    let rows = usize::from(model.pane_rows.max(1));
    let cols = usize::from(model.pane_cols.max(1));
    model
        .active_session()
        .map_or(rows, |s| page_lines(s, cols, rows))
}

fn handle_ask_key(key: KeyEvent, model: &mut Model) -> InputAction {
    if model.planner_busy {
        // Esc or Ctrl+T abort the in-flight request. Its result is dropped on arrival.
        let cancel = key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('t'));
        if cancel {
            model.cancel_planner();
            model.input_mode = InputMode::Shell;
            return InputAction::ModeSwitched;
        }
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
        KeyCode::Up | KeyCode::Char('k') => {
            nudge_review(model, -1, 0);
            InputAction::Ignored
        }
        KeyCode::Down | KeyCode::Char('j') => {
            nudge_review(model, 1, 0);
            InputAction::Ignored
        }
        KeyCode::PageUp => {
            let (rows, _) = review_window(model);
            nudge_review(model, -(rows.max(1) as isize), 0);
            InputAction::Ignored
        }
        KeyCode::PageDown => {
            let (rows, _) = review_window(model);
            nudge_review(model, rows.max(1) as isize, 0);
            InputAction::Ignored
        }
        KeyCode::Left => {
            nudge_review(model, 0, -4);
            InputAction::Ignored
        }
        KeyCode::Right => {
            nudge_review(model, 0, 4);
            InputAction::Ignored
        }
        KeyCode::Char('c')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            if model
                .current_proposal
                .as_ref()
                .is_some_and(|proposal| proposal.requires_confirmation)
            {
                model.proposal_confirmed = !model.proposal_confirmed;
                if model.proposal_confirmed {
                    model.review_error = None;
                }
            }
            InputAction::Ignored
        }
        // Ctrl/Alt chords (readline yank, etc.) never approve or cancel.
        KeyCode::Enter | KeyCode::Char('y') if !has_ctrl_or_alt(key) => try_approve(model),
        KeyCode::Esc | KeyCode::Char('n') if key.code == KeyCode::Esc || !has_ctrl_or_alt(key) => {
            model.input_mode = InputMode::Ask;
            model.clear_proposal();
            InputAction::CancelProposal
        }
        _ => InputAction::Ignored,
    }
}

fn has_ctrl_or_alt(key: KeyEvent) -> bool {
    key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn nudge_review(model: &mut Model, dy: isize, dx: isize) {
    let lines = model
        .current_proposal
        .as_ref()
        .map(|proposal| visible_command_lines(&proposal.command))
        .unwrap_or_default();
    let (rows, cols) = review_window(model);
    let max_y = lines.len().saturating_sub(rows);
    let max_x = widest_line(&lines).saturating_sub(cols);
    model.review_scroll = move_scroll(model.review_scroll, dy, max_y);
    model.review_scroll_x = move_scroll(model.review_scroll_x, dx, max_x);
}

fn move_scroll(current: usize, delta: isize, max: usize) -> usize {
    let next = if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize)
    };
    next.min(max)
}

/// Approve only when the whole command is on screen and confirmation, if required, is done.
/// Does not clear the proposal — execute does that after Ok.
fn try_approve(model: &mut Model) -> InputAction {
    let (Some(proposal), Some(session_id)) = (
        model.current_proposal.clone(),
        model.proposal_session_id.clone(),
    ) else {
        return InputAction::Ignored;
    };

    // A keypress buffered while the planner ran must not approve a command that
    // has not been on screen long enough to read.
    if !model.review_input_ready() {
        return InputAction::Ignored;
    }

    // The runtime refuses `execute` while a command holds the foreground, and
    // a command the user typed by hand (ssh, vim, make) holds it too. Say so
    // here, in the runtime's words, instead of after the approve.
    if let Some(msg) = model
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| s.busy_message())
    {
        model.review_error = Some(msg.to_string());
        return InputAction::Ignored;
    }

    // The proposal was written for the cwd at Ask time. If the shell has moved,
    // the command would run somewhere the review did not show.
    if let Some(drift) = model.cwd_drift(&proposal, &session_id) {
        model.review_error = Some(drift);
        return InputAction::Ignored;
    }

    let lines = visible_command_lines(&proposal.command);
    let (rows, cols) = review_window(model);
    let max_y = lines.len().saturating_sub(rows);
    let max_x = widest_line(&lines).saturating_sub(cols);
    model.review_scroll = model.review_scroll.min(max_y);
    model.review_scroll_x = model.review_scroll_x.min(max_x);
    if command_is_clipped(
        &lines,
        model.review_scroll,
        model.review_scroll_x,
        rows,
        cols,
    ) {
        model.review_error =
            Some("Command is clipped — scroll to the end before approve".to_string());
        return InputAction::Ignored;
    }
    if proposal.requires_confirmation && !model.proposal_confirmed {
        model.review_error = Some("Confirmation required — press c, then Enter".to_string());
        return InputAction::Ignored;
    }

    InputAction::ApproveProposal(proposal.command, session_id, proposal.id)
}

/// Raw play mode — nearly all keys forwarded to the game.
/// Only the escape chord (Ctrl+\) returns to Console.
/// Ctrl+Q also exits raw mode first, then quits.
fn handle_raw_play_key(
    key: KeyEvent,
    model: &mut Model,
    terminal_service: &TerminalService,
) -> InputAction {
    // Escape chord: Ctrl+\.
    // Windows crossterm 0.28 reports Char('\\') + CONTROL.
    // Unix crossterm 0.28 maps byte 0x1c to Char('4') + CONTROL.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('\\') | KeyCode::Char('4'))
    {
        model.input_mode = InputMode::Shell;
        return InputAction::ExitRawPlay;
    }

    // Ctrl+Q during raw play — exit raw mode, then ask. The child may be a
    // full-screen editor where Ctrl+Q is its own chord, so one press never quits
    // here; the prompt is drawn by Console after the terminal is restored.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        model.input_mode = InputMode::Shell;
        model.ask_confirm(PendingConfirm::Quit);
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
        let result = terminal_service.write(&session_id, &data);
        if let Err(err) = &result {
            // Raw Play draws no status line, so a dead PTY would swallow keys in
            // silence. Mark the session, show the reason in Console and leave.
            if let Some(idx) = model.session_index(&session_id) {
                model.sessions[idx].session_state = SessionState::Error(err.clone());
            }
            model.surface_session_result(result);
            model.input_mode = InputMode::Shell;
            return InputAction::ExitRawPlay;
        }
        model.surface_session_result(result);
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
            // Close the session at switcher_cursor, not the active one. The active
            // session stays selected; app removes the target by id.
            let target = model.sessions.get(model.switcher_cursor).map(|s| s.id.clone());
            model.close_switcher();
            match target {
                Some(id) if model.session_is_running(&id) => {
                    model.ask_confirm(PendingConfirm::CloseSession(id));
                    InputAction::Ignored
                }
                Some(id) => InputAction::CloseSessionAt(id),
                None => InputAction::Ignored,
            }
        }

        // Number keys 1-9 jump to the numbered row of the drawn window.
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let slot = (c as usize) - ('1' as usize);
            // Before the first draw there is no window; the whole list is shown.
            let (start, shown) = if model.switcher_rows == 0 {
                (0, count)
            } else {
                (model.switcher_start, model.switcher_rows)
            };
            let idx = start + slot;
            if slot < shown && idx < count {
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
        // Unix crossterm 0.28 encodes 0x1c..=0x1f as CONTROL + '4'..='7'.
        b'4' => Some(0x1c),
        b'5' => Some(0x1d),
        b'6' => Some(0x1e),
        b'7' => Some(0x1f),
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
        // Shift+Tab: crossterm reports BackTab on Windows and Unix.
        KeyCode::BackTab => alt_prefix(mods, "\x1b[Z"),
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
    use crate::model::{CommandProposal, SessionState};
    use commandui_runtime_core::events::NoopSink;
    use commandui_runtime_core::session::SessionRegistry;
    use std::sync::{Arc, Mutex};

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

    #[test]
    fn unix_high_controls_are_forwarded_as_bytes() {
        assert_eq!(key_to_bytes(press(KeyCode::Char('4'), KeyModifiers::CONTROL)), "\u{1c}");
        assert_eq!(key_to_bytes(press(KeyCode::Char('5'), KeyModifiers::CONTROL)), "\u{1d}");
        assert_eq!(key_to_bytes(press(KeyCode::Char('6'), KeyModifiers::CONTROL)), "\u{1e}");
        assert_eq!(key_to_bytes(press(KeyCode::Char('7'), KeyModifiers::CONTROL)), "\u{1f}");
        assert_eq!(key_to_bytes(press(KeyCode::Char('\\'), KeyModifiers::CONTROL)), "\u{1c}");
    }

    fn service() -> TerminalService {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(NoopSink);
        TerminalService::new(sessions, sink)
    }

    fn two_sessions() -> Model {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());
        model.sessions[0].session_state = SessionState::Active;
        model.sessions[1].session_state = SessionState::Active;
        model.sessions[0].exec_state = "ready".into();
        model.sessions[1].exec_state = "ready".into();
        model
    }

    fn proposal(command: &str, confirm: bool) -> CommandProposal {
        CommandProposal {
            id: "plan-9".to_string(),
            session_id: "s1".to_string(),
            source: "mock".to_string(),
            user_intent: "test".to_string(),
            command: command.to_string(),
            cwd: None,
            explanation: "because".to_string(),
            assumptions: vec![],
            confidence: 0.5,
            risk: "high".to_string(),
            destructive: false,
            requires_confirmation: confirm,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            expected_output: None,
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn raw_play_exits_on_windows_backslash_and_unix_ctrl_4() {
        let terminal = service();
        for code in [KeyCode::Char('\\'), KeyCode::Char('4')] {
            let mut model = two_sessions();
            model.input_mode = InputMode::RawPlay;
            let action = handle_key(press(code, KeyModifiers::CONTROL), &mut model, &terminal);
            assert_eq!(action, InputAction::ExitRawPlay);
            assert_eq!(model.input_mode, InputMode::Shell);
            assert!(model.status_line.is_none());
        }
    }

    #[test]
    fn raw_play_forwards_other_control_bytes() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::RawPlay;
        let action = handle_key(
            press(KeyCode::Char('6'), KeyModifiers::CONTROL),
            &mut model,
            &terminal,
        );
        // A failed write marks the session and returns to Console, where the
        // status line is drawn.
        assert_eq!(model.input_mode, InputMode::Shell);
        assert_eq!(model.active_index, 0);
        assert!(matches!(model.sessions[0].session_state, SessionState::Error(_)));
        let status = model.status_line.expect("write error must be surfaced");
        assert!(status.contains("Session not found"), "{status}");
        assert_eq!(action, InputAction::ExitRawPlay);

        let mut model = two_sessions();
        model.input_mode = InputMode::RawPlay;
        handle_key(
            press(KeyCode::Char('5'), KeyModifiers::CONTROL),
            &mut model,
            &terminal,
        );
        assert_eq!(model.input_mode, InputMode::Shell);
        assert_eq!(model.active_index, 0);
        assert!(model.status_line.is_some());
    }

    #[test]
    fn shell_next_and_prev_accept_windows_and_unix_events() {
        let terminal = service();
        for code in [KeyCode::Char(']'), KeyCode::Char('5')] {
            let mut model = two_sessions();
            let action = handle_key(press(code, KeyModifiers::CONTROL), &mut model, &terminal);
            assert_eq!(action, InputAction::NextSession);
            assert_eq!(model.active_session_id(), Some("s2"));
            assert!(model.status_line.is_none());
        }

        let mut model = two_sessions();
        model.switch_to(1);
        let action = handle_key(
            press(KeyCode::Char('['), KeyModifiers::CONTROL),
            &mut model,
            &terminal,
        );
        assert_eq!(action, InputAction::PrevSession);
        assert_eq!(model.active_session_id(), Some("s1"));

        let mut model = two_sessions();
        model.switch_to(1);
        let action = handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal);
        if ESC_IS_PREV_SESSION {
            // Unix crossterm reports Ctrl+[ as Esc.
            assert_eq!(action, InputAction::PrevSession);
            assert_eq!(model.active_session_id(), Some("s1"));
        } else {
            // Windows reports Ctrl+[ itself, so bare Esc is not a session switch:
            // it goes to the shell as ESC (the write fails here, with no PTY).
            assert_eq!(action, InputAction::Ignored);
            assert_eq!(model.active_session_id(), Some("s2"));
            assert!(model.status_line.as_deref().unwrap().contains("Session not found"));
        }

        let mut model = two_sessions();
        model.switch_to(1);
        model.show_help = true;
        let action = handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(!model.show_help);
        assert_eq!(model.active_session_id(), Some("s2"));
    }

    #[test]
    fn approve_stays_in_review_until_execute_and_blocks_while_clipped() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.review_rows = 4;
        model.review_cols = 80;
        let command = (0..12).map(|i| format!("echo {i}")).collect::<Vec<_>>().join("\n");
        model.set_proposal(proposal(&command, false), "s1".into());

        let blocked = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(blocked, InputAction::Ignored);
        assert_eq!(model.input_mode, InputMode::Review);
        assert_eq!(model.current_proposal.as_ref().unwrap().command, command);
        assert!(model.review_error.as_deref().unwrap().contains("clipped"));

        for _ in 0..6 {
            handle_key(press(KeyCode::PageDown, KeyModifiers::NONE), &mut model, &terminal);
        }
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(
            action,
            InputAction::ApproveProposal(command.clone(), "s1".into(), "plan-9".into())
        );
        assert_eq!(model.input_mode, InputMode::Review);
        assert!(model.current_proposal.is_some());
    }

    #[test]
    fn ctrl_and_alt_chords_never_approve_or_cancel() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.set_proposal(proposal("echo hi", false), "s1".into());
        for (c, m) in [
            ('y', KeyModifiers::CONTROL),
            ('y', KeyModifiers::ALT),
            ('n', KeyModifiers::CONTROL),
            ('n', KeyModifiers::ALT),
        ] {
            let action = handle_key(press(KeyCode::Char(c), m), &mut model, &terminal);
            assert_eq!(action, InputAction::Ignored);
            assert_eq!(model.input_mode, InputMode::Review);
            assert!(model.current_proposal.is_some());
        }
        let action = handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal);
        assert!(matches!(action, InputAction::ApproveProposal(_, _, _)));
    }

    #[test]
    fn approve_is_ignored_during_the_debounce_after_a_proposal_arrives() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.set_proposal(proposal("echo hi", false), "s1".into());
        model.arm_review_debounce();
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        model.review_armed_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert!(matches!(action, InputAction::ApproveProposal(_, _, _)));
    }

    #[test]
    fn approve_is_refused_when_the_session_cwd_moved() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.sessions[0].cwd = Some("/new".into());
        let mut p = proposal("rm -rf build", false);
        p.cwd = Some("/old".into());
        model.set_proposal(p, "s1".into());
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.review_error.as_deref().unwrap().contains("Directory changed"));
        model.sessions[0].cwd = Some("/old".into());
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert!(matches!(action, InputAction::ApproveProposal(_, _, _)));
    }

    #[test]
    fn switcher_close_leaves_the_active_session_selected() {
        let mut model = two_sessions();
        model.add_session("s3".into(), "C".into());
        model.switch_to(2);
        assert!(model.remove_session_by_id("s1"));
        assert_eq!(model.active_session_id(), Some("s3"));
        assert!(model.remove_session_by_id("s3"));
        assert_eq!(model.active_session_id(), Some("s2"));
    }

    #[test]
    fn approve_honors_requires_confirmation_and_keeps_the_raw_command() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.review_rows = 8;
        model.review_cols = 80;
        let command = "echo a\necho b\r";
        model.set_proposal(proposal(command, true), "s1".into());

        let blocked = handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(blocked, InputAction::Ignored);
        assert!(model.review_error.as_deref().unwrap().contains("onfirmation"));
        assert!(model.current_proposal.is_some());

        handle_key(press(KeyCode::Char('c'), KeyModifiers::NONE), &mut model, &terminal);
        assert!(model.proposal_confirmed);
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(
            action,
            InputAction::ApproveProposal(command.to_string(), "s1".into(), "plan-9".into())
        );
        assert_eq!(model.input_mode, InputMode::Review);
        assert_eq!(model.current_proposal.as_ref().unwrap().command, command);
    }

    #[test]
    fn wide_command_is_blocked_until_scrolled_right() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.review_rows = 6;
        model.review_cols = 8;
        model.set_proposal(proposal("echo hello-from-the-shell", false), "s1".into());
        let blocked = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(blocked, InputAction::Ignored);
        assert!(model.review_error.as_deref().unwrap().contains("clipped"));

        for _ in 0..12 {
            handle_key(press(KeyCode::Right, KeyModifiers::NONE), &mut model, &terminal);
        }
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert!(matches!(action, InputAction::ApproveProposal(_, _, _)));
    }

    #[test]
    fn shell_write_interrupt_and_resync_errors_are_surfaced() {
        let terminal = service();
        let mut model = two_sessions();
        let action = handle_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.status_line.as_deref().unwrap().contains("Session not found"));

        model.sessions[0].exec_state = "running".into();
        model.status_line = None;
        let action = handle_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.status_line.as_deref().unwrap().contains("Session not found"));

        model.sessions[0].exec_state = "ready".into();
        model.status_line = None;
        let action = handle_key(press(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.status_line.as_deref().unwrap().contains("Session not found"));
    }

    #[test]
    fn shell_chords_scroll_help_and_mode_changes() {
        let terminal = service();

        let mut model = Model::new();
        assert_eq!(
            handle_key(press(KeyCode::Char('q'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Quit
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );

        let mut model = two_sessions();
        model.sessions.pop();
        // The last session can close.
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::CloseSession
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut Model::new(), &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::Char(']'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );

        let mut model = two_sessions();
        model.sessions[0].terminal_lines = vec!["a".into(), "b".into(), "c".into()];
        model.pane_rows = 1;
        assert_eq!(
            handle_key(press(KeyCode::PageUp, KeyModifiers::SHIFT), &mut model, &terminal),
            InputAction::Scrolled
        );
        assert!(model.sessions[0].scroll_offset > 0);
        assert_eq!(
            handle_key(press(KeyCode::PageDown, KeyModifiers::SHIFT), &mut model, &terminal),
            InputAction::Scrolled
        );

        model.show_help = true;
        assert_eq!(
            handle_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert!(model.show_help);
        assert_eq!(
            handle_key(press(KeyCode::F(1), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert!(!model.show_help);
        handle_key(press(KeyCode::F(1), KeyModifiers::NONE), &mut model, &terminal);
        assert!(model.show_help);

        let mut model = two_sessions();
        model.sessions[0].session_state = SessionState::Booting;
        assert_eq!(
            handle_key(press(KeyCode::Char('g'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('t'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );

        let mut model = two_sessions();
        assert_eq!(
            handle_key(press(KeyCode::Char('g'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::EnterRawPlay
        );
        let mut model = two_sessions();
        assert_eq!(
            handle_key(press(KeyCode::Char('s'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::ModeSwitched
        );
        assert_eq!(model.input_mode, InputMode::Switcher);
        let mut model = two_sessions();
        assert_eq!(
            handle_key(press(KeyCode::Char('t'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::ModeSwitched
        );
        assert_eq!(model.input_mode, InputMode::Ask);
        let mut model = two_sessions();
        assert_eq!(
            handle_key(press(KeyCode::Char('n'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::CreateSession
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::CloseSession
        );

        let mut model = two_sessions();
        model.sessions[0].exec_state = "interrupting".into();
        let action = handle_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);

        let mut model = two_sessions();
        model.sessions[0].scroll_offset = 2;
        model.sessions[0].terminal_lines = vec!["a".into(), "b".into(), "c".into()];
        handle_key(press(KeyCode::Char('z'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(model.sessions[0].scroll_offset, 0);

        let mut model = two_sessions();
        assert_eq!(
            handle_key(press(KeyCode::F(20), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::PageUp, KeyModifiers::SHIFT), &mut Model::new(), &terminal),
            InputAction::Scrolled
        );
    }

    #[test]
    fn ask_review_switcher_and_raw_keys_cover_each_arm() {
        let terminal = service();

        let mut model = two_sessions();
        model.input_mode = InputMode::Ask;
        model.planner_busy = true;
        assert_eq!(
            handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        model.planner_busy = false;
        assert_eq!(
            handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        model.composer_text = "  list files  ".into();
        model.composer_cursor = model.composer_text.len();
        assert_eq!(
            handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::SubmitIntent("list files".into())
        );
        assert!(model.planner_busy);

        let mut model = two_sessions();
        model.input_mode = InputMode::Ask;
        model.composer_text = "ab".into();
        model.composer_cursor = 2;
        handle_key(press(KeyCode::Backspace, KeyModifiers::NONE), &mut model, &terminal);
        handle_key(press(KeyCode::Left, KeyModifiers::NONE), &mut model, &terminal);
        handle_key(press(KeyCode::Right, KeyModifiers::NONE), &mut model, &terminal);
        handle_key(press(KeyCode::Char('日'), KeyModifiers::NONE), &mut model, &terminal);
        handle_key(press(KeyCode::Char('u'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert!(model.composer_text.is_empty());
        assert_eq!(
            handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::ModeSwitched
        );
        model.input_mode = InputMode::Ask;
        assert_eq!(
            handle_key(press(KeyCode::Char('t'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::ModeSwitched
        );
        model.input_mode = InputMode::Ask;
        assert_eq!(
            handle_key(press(KeyCode::F(1), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );

        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        assert_eq!(
            handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        model.set_proposal(proposal("echo hi", false), "s1".into());
        model.review_rows = 6;
        model.review_cols = 80;
        for code in [KeyCode::Up, KeyCode::Down, KeyCode::PageUp, KeyCode::PageDown, KeyCode::Left, KeyCode::Right, KeyCode::Char('k'), KeyCode::Char('j')] {
            handle_key(press(code, KeyModifiers::NONE), &mut model, &terminal);
        }
        handle_key(press(KeyCode::Char('c'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(
            handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::ApproveProposal("echo hi".into(), "s1".into(), "plan-9".into())
        );
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.review_rows = 6;
        model.review_cols = 80;
        model.set_proposal(proposal("echo hi", true), "s1".into());
        handle_key(press(KeyCode::Char('c'), KeyModifiers::NONE), &mut model, &terminal);
        assert!(model.proposal_confirmed);
        handle_key(press(KeyCode::Char('c'), KeyModifiers::NONE), &mut model, &terminal);
        assert!(!model.proposal_confirmed);
        assert_eq!(
            handle_key(press(KeyCode::Char('n'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::CancelProposal
        );
        assert_eq!(model.input_mode, InputMode::Ask);

        let mut model = Model::new();
        model.input_mode = InputMode::Switcher;
        assert_eq!(
            handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::ModeSwitched
        );

        let mut model = two_sessions();
        model.open_switcher();
        assert_eq!(
            handle_key(press(KeyCode::Up, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(model.switcher_cursor, 1);
        assert_eq!(
            handle_key(press(KeyCode::Down, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('2'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::NextSession
        );
        let mut model = two_sessions();
        model.open_switcher();
        assert_eq!(
            handle_key(press(KeyCode::Char('9'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::ModeSwitched
        );
        model.open_switcher();
        assert_eq!(
            handle_key(press(KeyCode::Char('s'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::ModeSwitched
        );
        model.open_switcher();
        assert_eq!(
            handle_key(press(KeyCode::Char('n'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::CreateSession
        );
        let mut model = two_sessions();
        model.sessions.pop();
        model.open_switcher();
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::CloseSessionAt(model.sessions[0].id.clone())
        );
        let mut model = two_sessions();
        model.open_switcher();
        model.switcher_cursor = 1;
        let second = model.sessions[1].id.clone();
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::CloseSessionAt(second)
        );
        let mut model = two_sessions();
        model.open_switcher();
        assert_eq!(
            handle_key(press(KeyCode::F(2), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );

        let mut model = two_sessions();
        model.input_mode = InputMode::RawPlay;
        model.sessions[0].session_state = SessionState::Closed;
        assert_eq!(
            handle_raw_play_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::ExitRawPlay
        );
        let mut model = two_sessions();
        model.input_mode = InputMode::RawPlay;
        // Ctrl+Q in Raw Play leaves Raw Play and asks; it never quits on one press.
        assert_eq!(
            handle_key(press(KeyCode::Char('q'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::ExitRawPlay
        );
        assert!(!model.should_quit);
        assert_eq!(model.pending_confirm, Some(PendingConfirm::Quit));
        assert_eq!(model.input_mode, InputMode::Shell);
        assert_eq!(
            handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Quit
        );
        let mut model = two_sessions();
        model.input_mode = InputMode::RawPlay;
        assert_eq!(
            handle_raw_play_key(press(KeyCode::F(20), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        let mut model = Model::new();
        model.input_mode = InputMode::RawPlay;
        assert_eq!(
            handle_raw_play_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::ExitRawPlay
        );
    }

    #[test]
    fn key_bytes_cover_specials_and_modifiers() {
        let mods = [
            KeyModifiers::NONE,
            KeyModifiers::SHIFT,
            KeyModifiers::ALT,
            KeyModifiers::CONTROL,
            KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::CONTROL,
        ];
        let codes = [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::Insert,
            KeyCode::Delete,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Enter,
            KeyCode::Backspace,
            KeyCode::Tab,
            KeyCode::Esc,
            KeyCode::F(1),
            KeyCode::F(12),
            KeyCode::F(13),
            KeyCode::Null,
        ];
        for mods in mods {
            for code in codes {
                let _ = key_to_bytes(press(code, mods));
            }
        }
        assert_eq!(key_to_bytes(press(KeyCode::Char(' '), KeyModifiers::CONTROL)), "\u{0}");
        assert_eq!(key_to_bytes(press(KeyCode::Char('@'), KeyModifiers::CONTROL)), "\u{0}");
        assert!(key_to_bytes(press(KeyCode::Char('日'), KeyModifiers::CONTROL)).is_empty());
        assert_eq!(key_to_bytes(press(KeyCode::Char('x'), KeyModifiers::ALT | KeyModifiers::CONTROL)).chars().next(), Some('\u{1b}'));
        assert_eq!(key_to_bytes(press(KeyCode::Enter, KeyModifiers::ALT)), "\u{1b}\r");
        assert_eq!(key_to_bytes(press(KeyCode::Char('a'), KeyModifiers::NONE)), "a");
    }

    #[test]
    fn shift_tab_is_the_backtab_sequence() {
        assert_eq!(key_to_bytes(press(KeyCode::BackTab, KeyModifiers::SHIFT)), "\u{1b}[Z");
        assert_eq!(key_to_bytes(press(KeyCode::BackTab, KeyModifiers::NONE)), "\u{1b}[Z");
        assert_eq!(
            key_to_bytes(press(KeyCode::BackTab, KeyModifiers::SHIFT | KeyModifiers::ALT)),
            "\u{1b}\u{1b}[Z"
        );
        assert_eq!(key_to_bytes(press(KeyCode::Tab, KeyModifiers::NONE)), "\t");
    }

    #[test]
    fn esc_or_ctrl_t_cancels_a_generating_proposal() {
        let terminal = service();
        for key in [
            press(KeyCode::Esc, KeyModifiers::NONE),
            press(KeyCode::Char('t'), KeyModifiers::CONTROL),
        ] {
            let mut model = two_sessions();
            model.input_mode = InputMode::Ask;
            model.planner_busy = true;
            let before = model.planner_epoch;
            let action = handle_key(key, &mut model, &terminal);
            assert_eq!(action, InputAction::ModeSwitched);
            assert_eq!(model.input_mode, InputMode::Shell);
            assert!(!model.planner_busy);
            // The epoch moved on, so the in-flight result will be dropped.
            assert_ne!(model.planner_epoch, before);
        }

        // Other keys are still swallowed while busy.
        let mut model = two_sessions();
        model.input_mode = InputMode::Ask;
        model.planner_busy = true;
        let action = handle_key(press(KeyCode::Char('x'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.planner_busy);
        assert!(model.composer_text.is_empty());
    }

    #[test]
    fn esc_without_the_prev_session_binding_reaches_the_shell() {
        let terminal = service();
        // One session: Esc never switches, on any platform. The shell gets ESC
        // (the write is attempted and fails here because there is no PTY).
        let mut model = two_sessions();
        model.sessions.pop();
        let action = handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.status_line.as_deref().unwrap().contains("Session not found"));
        assert_eq!(key_to_bytes(press(KeyCode::Esc, KeyModifiers::NONE)), "\u{1b}");
    }

    #[test]
    fn approval_targets_the_proposal_owner_not_the_active_session() {
        let terminal = service();
        let mut model = two_sessions();
        model.set_proposal(proposal("echo hi", false), "s2".into());
        model.input_mode = InputMode::Review;
        model.review_rows = 6;
        model.review_cols = 80;
        assert_eq!(model.active_session_id(), Some("s1"));
        let action = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(
            action,
            InputAction::ApproveProposal("echo hi".into(), "s2".into(), "plan-9".into())
        );
    }

    #[test]
    fn fullwidth_command_is_blocked_by_display_width_not_char_count() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::Review;
        model.review_rows = 6;
        // The quoted line is 14 chars but 24 cells wide, in a 20-column pane.
        model.review_cols = 20;
        let command = "\u{65e5}".repeat(10);
        model.set_proposal(proposal(&command, false), "s1".into());
        assert!(command.chars().count() + 4 < 20);
        let blocked = handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(blocked, InputAction::Ignored);
        assert!(model.review_error.as_deref().unwrap().contains("clipped"));
    }

    #[test]
    fn ctrl_w_on_a_running_session_asks_and_only_a_bare_y_closes_it() {
        let terminal = service();
        let ctrl_w = || press(KeyCode::Char('w'), KeyModifiers::CONTROL);

        // Idle: closes at once, as before.
        let mut model = two_sessions();
        assert_eq!(handle_key(ctrl_w(), &mut model, &terminal), InputAction::CloseSession);
        assert!(model.pending_confirm.is_none());

        // Running: asks, closes nothing.
        let mut model = two_sessions();
        model.sessions[0].exec_state = "running".into();
        assert_eq!(handle_key(ctrl_w(), &mut model, &terminal), InputAction::Ignored);
        assert_eq!(
            model.pending_confirm,
            Some(PendingConfirm::CloseSession("s1".into()))
        );
        assert!(model.status_line.as_deref().unwrap().contains("running command"));
        assert_eq!(model.session_count(), 2);

        // y with a modifier cancels; so does any other key. Neither is forwarded.
        for key in [
            press(KeyCode::Char('y'), KeyModifiers::CONTROL),
            press(KeyCode::Char('Y'), KeyModifiers::SHIFT),
            press(KeyCode::Char('n'), KeyModifiers::NONE),
            press(KeyCode::Enter, KeyModifiers::NONE),
        ] {
            let mut model = two_sessions();
            model.sessions[0].exec_state = "interrupting".into();
            handle_key(ctrl_w(), &mut model, &terminal);
            assert_eq!(handle_key(key, &mut model, &terminal), InputAction::Ignored);
            assert!(model.pending_confirm.is_none());
            assert!(model.status_line.is_none());
        }

        // A bare y proceeds, targeting the session that was asked about.
        let mut model = two_sessions();
        model.sessions[0].exec_state = "running".into();
        handle_key(ctrl_w(), &mut model, &terminal);
        assert_eq!(
            handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::CloseSessionAt("s1".into())
        );
        assert!(model.pending_confirm.is_none());
    }

    #[test]
    fn ctrl_q_asks_while_any_session_runs_and_quits_when_all_are_idle() {
        let terminal = service();
        let ctrl_q = || press(KeyCode::Char('q'), KeyModifiers::CONTROL);

        // The running session is not the active one: Ctrl+Q still asks.
        let mut model = two_sessions();
        model.sessions[1].exec_state = "running".into();
        assert_eq!(handle_key(ctrl_q(), &mut model, &terminal), InputAction::Ignored);
        assert_eq!(model.pending_confirm, Some(PendingConfirm::Quit));
        assert_eq!(
            handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Ignored
        );
        assert!(model.pending_confirm.is_none());
        handle_key(ctrl_q(), &mut model, &terminal);
        assert_eq!(
            handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::Quit
        );

        // Also from the run selector, which leaves so the prompt shows.
        let mut model = two_sessions();
        model.sessions[0].exec_state = "running".into();
        model.open_switcher();
        assert_eq!(handle_key(ctrl_q(), &mut model, &terminal), InputAction::Ignored);
        assert_eq!(model.input_mode, InputMode::Shell);
        assert_eq!(model.pending_confirm, Some(PendingConfirm::Quit));

        // All idle: quits at once.
        let mut model = two_sessions();
        assert_eq!(handle_key(ctrl_q(), &mut model, &terminal), InputAction::Quit);
    }

    #[test]
    fn closing_a_running_session_from_the_run_selector_asks_first() {
        let terminal = service();
        let mut model = two_sessions();
        model.sessions[1].exec_state = "running".into();
        model.open_switcher();
        model.switcher_cursor = 1;
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            model.pending_confirm,
            Some(PendingConfirm::CloseSession("s2".into()))
        );
        assert_eq!(
            handle_key(press(KeyCode::Char('y'), KeyModifiers::NONE), &mut model, &terminal),
            InputAction::CloseSessionAt("s2".into())
        );
    }

    #[test]
    fn switcher_digit_keys_jump_to_rows_in_the_drawn_window() {
        let terminal = service();
        let mut model = Model::new();
        for i in 0..12 {
            model.add_session(format!("s{i}"), format!("S{i}"));
        }
        model.open_switcher();
        // The overlay showed rows 5..9 (4 rows), so key 2 is session index 6.
        model.switcher_start = 5;
        model.switcher_rows = 4;
        model.switcher_cursor = 8;
        let action = handle_key(press(KeyCode::Char('2'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::NextSession);
        assert_eq!(model.active_index, 6);

        // A key past the drawn rows does nothing.
        let mut model = Model::new();
        for i in 0..12 {
            model.add_session(format!("s{i}"), format!("S{i}"));
        }
        model.open_switcher();
        model.switcher_start = 0;
        model.switcher_rows = 4;
        let action = handle_key(press(KeyCode::Char('5'), KeyModifiers::NONE), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert_eq!(model.input_mode, InputMode::Switcher);
    }

    #[test]
    fn a_hand_typed_command_gets_the_close_and_quit_confirms() {
        let terminal = service();
        let mut model = two_sessions();
        model.sessions[0].exec_state = "userRunning".into();
        assert_eq!(
            handle_key(press(KeyCode::Char('w'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(
            model.pending_confirm,
            Some(PendingConfirm::CloseSession("s1".into()))
        );
        model.pending_confirm = None;
        assert_eq!(
            handle_key(press(KeyCode::Char('q'), KeyModifiers::CONTROL), &mut model, &terminal),
            InputAction::Ignored
        );
        assert_eq!(model.pending_confirm, Some(PendingConfirm::Quit));
    }

    #[test]
    fn review_refuses_approval_while_a_program_holds_the_foreground() {
        let terminal = service();
        for (state, expected) in [
            ("userRunning", crate::model::USER_RUNNING_MESSAGE),
            ("running", crate::model::RUNNING_MESSAGE),
        ] {
            let mut model = two_sessions();
            model.input_mode = InputMode::Review;
            model.review_rows = 8;
            model.review_cols = 80;
            model.set_proposal(proposal("ls -la", false), "s1".into());
            model.sessions[0].exec_state = state.into();
            let action =
                handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
            assert_eq!(action, InputAction::Ignored);
            assert_eq!(model.review_error.as_deref(), Some(expected));
            assert!(model.current_proposal.is_some());

            model.sessions[0].exec_state = "ready".into();
            let action =
                handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &mut model, &terminal);
            assert!(matches!(action, InputAction::ApproveProposal(_, _, _)));
        }
    }

    #[test]
    fn ctrl_h_is_backspace_not_help_and_ctrl_slash_opens_help() {
        let terminal = service();
        let mut model = two_sessions();
        handle_key(press(KeyCode::Char('h'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert!(!model.show_help);
        handle_key(press(KeyCode::Char('/'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert!(model.show_help);
        handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &mut model, &terminal);
        assert!(!model.show_help);
        handle_key(press(KeyCode::Char('7'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert!(model.show_help);
    }

    #[test]
    fn ctrl_bracket_is_ignored_with_one_session_in_shell_mode() {
        let terminal = service();
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.sessions[0].session_state = SessionState::Active;
        model.sessions[0].exec_state = "ready".into();
        let action = handle_key(press(KeyCode::Char(']'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert_eq!(model.active_session_id(), Some("s1"));
        assert!(model.status_line.is_none());
    }

    #[test]
    fn ctrl_bracket_is_forwarded_to_the_shell_in_raw_play() {
        let terminal = service();
        let mut model = two_sessions();
        model.input_mode = InputMode::RawPlay;
        let action = handle_key(press(KeyCode::Char(']'), KeyModifiers::CONTROL), &mut model, &terminal);
        // Not a session switch: the byte went to the runtime, whose refusal
        // (no such session in the test service) marks the session.
        assert_ne!(action, InputAction::NextSession);
        assert_eq!(model.active_session_id(), Some("s1"));
        assert!(matches!(model.sessions[0].session_state, SessionState::Error(_)));
    }

    #[test]
    fn a_booting_session_takes_resync_but_not_typed_keys() {
        let terminal = service();
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        assert_eq!(model.sessions[0].session_state, SessionState::Booting);
        // The test service has no such session, so the write reaches the runtime
        // and its refusal is surfaced; an ignored key would leave no status.
        let action = handle_key(press(KeyCode::Char('r'), KeyModifiers::CONTROL), &mut model, &terminal);
        assert_eq!(action, InputAction::Ignored);
        assert!(model.status_line.as_deref().unwrap().contains("Session not found"));
        model.status_line = None;
        // Typed text and Ctrl+C would interleave with the bootstrap echo.
        for k in [
            press(KeyCode::Char('a'), KeyModifiers::NONE),
            press(KeyCode::Enter, KeyModifiers::NONE),
            press(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(handle_key(k, &mut model, &terminal), InputAction::Ignored);
            assert!(model.status_line.is_none());
        }
    }
}
