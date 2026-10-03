//! Console UI model — multi-session with targeting truth.
//!
//! Each session has its own buffer, scroll state, cwd, exec state.
//! The model holds all sessions and an active index.
//! Event routing matches session_id to update the correct session.
//! Input/actions always target the active session.

use commandui_runtime_core::events::RuntimeEvent;
pub use commandui_runtime_planner::CommandProposal;

/// Maximum lines retained per session scrollback buffer.
const MAX_LINES: usize = 10_000;

/// Maximum bytes kept for one line, finished or still open. A newline-free
/// stream (progress bar, binary dump) cannot grow memory past this.
const MAX_LINE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Shell,
    Ask,
    Review,
    /// Run selector overlay is open.
    Switcher,
    /// Raw play mode — game owns the terminal, Console steps back.
    /// PTY output goes directly to stdout. All keys forwarded to PTY.
    /// Only the escape chord returns to Console.
    RawPlay,
}

/// Session lifecycle state.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Booting,
    Active,
    Closed,
    Error(String),
}

/// Per-session state — each session has its own independent truth.
pub struct SessionModel {
    pub id: String,
    pub label: String,
    pub terminal_lines: Vec<String>,
    pub cwd: Option<String>,
    /// The shell this session runs, from the runtime's session summary.
    pub shell: Option<String>,
    pub exec_state: String,
    pub session_state: SessionState,
    pub scroll_offset: usize,
    pub lines_dropped: usize,
    /// True when this session received output while not active.
    /// Cleared when session becomes the active session.
    pub has_unread: bool,
    /// Trailing fragment that did not end in a newline. The next chunk appends here.
    pub line_remainder: String,
    /// A CR arrived. The next appended character replaces the current logical line.
    /// CR followed by LF is a line break and does not erase the line.
    pub line_cr_pending: bool,
    /// Cursor inside `line_remainder`, as a byte offset on a char boundary. Output
    /// writes overwrite at the cursor; BS, CSI C/D/G move it; CSI K/P/@ edit around it.
    pub line_cursor: usize,
    /// Parameter bytes of the CSI sequence being parsed.
    csi_params: String,
    /// Bytes discarded because one line grew past `MAX_LINE_BYTES` with no newline.
    pub bytes_dropped: usize,
    /// Escape-sequence parser state, kept across chunks.
    esc_state: EscState,
}

/// Where the ingest parser is inside an ANSI escape sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EscState {
    Ground,
    /// Saw ESC.
    Esc,
    /// ESC followed by a charset or similar intermediate; one more byte to swallow.
    EscSkip,
    /// Inside CSI (`ESC [`), waiting for the final byte.
    Csi,
    /// Inside OSC (`ESC ]`), waiting for BEL or ST.
    Osc,
    /// Saw ESC inside OSC; `\` ends it.
    OscEsc,
}

impl SessionModel {
    pub fn new(id: String, label: String) -> Self {
        Self {
            id,
            label,
            terminal_lines: Vec::new(),
            cwd: None,
            shell: None,
            exec_state: "booting".to_string(),
            session_state: SessionState::Booting,
            scroll_offset: 0,
            lines_dropped: 0,
            has_unread: false,
            line_remainder: String::new(),
            line_cr_pending: false,
            line_cursor: 0,
            csi_params: String::new(),
            bytes_dropped: 0,
            esc_state: EscState::Ground,
        }
    }

    /// State badge for the run selector — play-aware, not shell-generic.
    pub fn state_badge(&self) -> &'static str {
        match &self.session_state {
            SessionState::Booting => "BOOT",
            SessionState::Closed => "DONE",
            SessionState::Error(_) => "ERROR",
            SessionState::Active => match self.exec_state.as_str() {
                "running" => "RUNNING",
                "interrupting" => "STOPPING",
                _ => "IDLE",
            },
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.session_state, SessionState::Active)
    }

    /// A command is running (or being stopped) in this session.
    pub fn has_running_command(&self) -> bool {
        matches!(self.exec_state.as_str(), "running" | "interrupting")
    }

    pub fn can_accept_input(&self) -> bool {
        matches!(self.session_state, SessionState::Active)
    }

    pub fn scroll_up(&mut self, lines: usize) {
        let max_offset = self.terminal_lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Join PTY chunks into logical lines.
    ///
    /// A chunk that does not end in `\n` stays in `line_remainder` and the next
    /// chunk appends to it. Empty lines are kept. `\r` clears the current
    /// logical line only when a later character is appended, so `\r\n` stays
    /// one line break.
    ///
    /// The open line has a cursor. Backspace moves it left (readline also uses BS
    /// as plain cursor-left), output overwrites at the cursor, and CSI K, D, C, G,
    /// P and @ erase, move, delete and insert, so a mid-line edit leaves the line
    /// the shell holds. A readline `\b \b` erase ends with only blanks after the
    /// cursor, which are dropped. DEL deletes the previous character. Other CSI,
    /// OSC and charset escape sequences are removed before the line is stored. While the user is
    /// scrolled up, `scroll_offset` grows with each new line so the viewport
    /// stays on the same lines.
    fn ingest_terminal_chunk(&mut self, text: &str) {
        for ch in text.chars() {
            self.ingest_char(ch);
        }
        if self.terminal_lines.len() > MAX_LINES {
            let excess = self.terminal_lines.len() - MAX_LINES;
            self.terminal_lines.drain(..excess);
            self.lines_dropped += excess;
        }
        // The offset counts from the tail, so a front trim does not move the
        // lines it points at. It only has to stay inside the buffer.
        self.scroll_offset = self
            .scroll_offset
            .min(self.terminal_lines.len().saturating_sub(1));
    }

    fn ingest_char(&mut self, ch: char) {
        match self.esc_state {
            EscState::Ground => {}
            EscState::Esc => {
                self.esc_state = match ch {
                    '[' => {
                        self.csi_params.clear();
                        EscState::Csi
                    }
                    ']' => EscState::Osc,
                    '(' | ')' | '*' | '+' | '#' | '%' => EscState::EscSkip,
                    // DCS, SOS, PM and APC carry a string body ended by BEL or ST,
                    // handled by the same state as OSC.
                    'P' | 'X' | '^' | '_' => EscState::Osc,
                    _ => EscState::Ground,
                };
                return;
            }
            EscState::EscSkip => {
                self.esc_state = EscState::Ground;
                return;
            }
            EscState::Csi => {
                match ch {
                    '\u{40}'..='\u{7e}' => {
                        self.esc_state = EscState::Ground;
                        self.dispatch_csi(ch);
                        return;
                    }
                    '\u{20}'..='\u{3f}' => {
                        if self.csi_params.len() < 32 {
                            self.csi_params.push(ch);
                        }
                        return;
                    }
                    // Anything else cancels the sequence and is handled as text.
                    _ => self.esc_state = EscState::Ground,
                }
            }
            EscState::Osc => {
                match ch {
                    '\u{7}' => {
                        self.esc_state = EscState::Ground;
                        return;
                    }
                    '\u{1b}' => {
                        self.esc_state = EscState::OscEsc;
                        return;
                    }
                    // A newline ends a runaway OSC so output is not swallowed.
                    '\n' => self.esc_state = EscState::Ground,
                    _ => return,
                }
            }
            EscState::OscEsc => {
                if ch == '\\' {
                    self.esc_state = EscState::Ground;
                    return;
                }
                self.esc_state = EscState::Esc;
                self.ingest_char(ch);
                return;
            }
        }

        match ch {
            '\u{1b}' => self.esc_state = EscState::Esc,
            '\n' => {
                self.line_cr_pending = false;
                self.trim_blank_tail();
                self.line_cursor = 0;
                let line = std::mem::take(&mut self.line_remainder);
                self.terminal_lines.push(line);
                if self.scroll_offset > 0 {
                    self.scroll_offset += 1;
                }
            }
            '\r' => {
                self.line_cr_pending = true;
            }
            '\u{8}' => {
                self.cursor_left(1);
                self.trim_blank_tail();
            }
            '\u{7f}' => {
                self.clamp_cursor();
                if let Some(c) = self.line_remainder[..self.line_cursor].chars().next_back() {
                    let at = self.line_cursor - c.len_utf8();
                    self.line_remainder.drain(at..self.line_cursor);
                    self.line_cursor = at;
                }
            }
            // Bell and other C0 controls draw nothing; tab is kept.
            c if c.is_control() && c != '\t' => {}
            other => self.put_char(other),
        }
    }

    fn clamp_cursor(&mut self) {
        let len = self.line_remainder.len();
        if self.line_cursor > len {
            self.line_cursor = len;
        }
        while !self.line_remainder.is_char_boundary(self.line_cursor) {
            self.line_cursor -= 1;
        }
    }

    /// Write one printable character at the cursor, overwriting what is there.
    fn put_char(&mut self, ch: char) {
        if self.line_cr_pending {
            self.line_remainder.clear();
            self.line_cursor = 0;
            self.line_cr_pending = false;
        }
        self.clamp_cursor();
        let len = self.line_remainder.len();
        if self.line_cursor >= len {
            if len + ch.len_utf8() > MAX_LINE_BYTES {
                self.bytes_dropped += ch.len_utf8();
            } else {
                self.line_remainder.push(ch);
                self.line_cursor = self.line_remainder.len();
            }
            return;
        }
        let next_len = self.line_remainder[self.line_cursor..]
            .chars()
            .next()
            .map_or(0, char::len_utf8);
        if len - next_len + ch.len_utf8() > MAX_LINE_BYTES {
            self.bytes_dropped += ch.len_utf8();
            return;
        }
        let mut buf = [0u8; 4];
        self.line_remainder.replace_range(
            self.line_cursor..self.line_cursor + next_len,
            ch.encode_utf8(&mut buf),
        );
        self.line_cursor += ch.len_utf8();
    }

    fn cursor_left(&mut self, n: usize) {
        self.clamp_cursor();
        for _ in 0..n {
            match self.line_remainder[..self.line_cursor].chars().next_back() {
                Some(c) => self.line_cursor -= c.len_utf8(),
                None => break,
            }
        }
    }

    fn cursor_right(&mut self, n: usize) {
        self.clamp_cursor();
        for _ in 0..n {
            match self.line_remainder[self.line_cursor..].chars().next() {
                Some(c) => self.line_cursor += c.len_utf8(),
                None => break,
            }
        }
    }

    /// Drop blanks after the cursor. A `\b \b` erase leaves them; a terminal
    /// shows them as nothing.
    fn trim_blank_tail(&mut self) {
        self.clamp_cursor();
        if self.line_remainder[self.line_cursor..]
            .chars()
            .all(|c| c == ' ')
        {
            self.line_remainder.truncate(self.line_cursor);
        }
    }

    /// Apply a CSI final byte to the open line. Only the sequences that edit or
    /// move within a line matter here; colour and mode sequences draw nothing.
    fn dispatch_csi(&mut self, fin: char) {
        let params = std::mem::take(&mut self.csi_params);
        if !matches!(fin, 'K' | 'D' | 'C' | 'G' | 'P' | '@') {
            return;
        }
        if !params.chars().all(|c| c.is_ascii_digit() || c == ';') {
            return;
        }
        // A pending CR is a move to column 0; the sequence applies from there.
        if self.line_cr_pending {
            self.line_cursor = 0;
            self.line_cr_pending = false;
        }
        let first: Option<usize> = params.split(';').next().and_then(|p| p.parse().ok());
        let n = first.unwrap_or(1).clamp(1, 1000);
        self.clamp_cursor();
        match fin {
            'D' => self.cursor_left(n),
            'C' => self.cursor_right(n),
            'G' => {
                self.line_cursor = 0;
                self.cursor_right(first.unwrap_or(1).clamp(1, 1000) - 1);
            }
            'K' => match first.unwrap_or(0) {
                0 => self.line_remainder.truncate(self.line_cursor),
                1 => {
                    let blanks =
                        " ".repeat(self.line_remainder[..self.line_cursor].chars().count());
                    self.line_remainder
                        .replace_range(..self.line_cursor, &blanks);
                    self.line_cursor = blanks.len();
                }
                2 => {
                    self.line_remainder.clear();
                    self.line_cursor = 0;
                }
                _ => {}
            },
            'P' => {
                let mut end = self.line_cursor;
                for _ in 0..n {
                    match self.line_remainder[end..].chars().next() {
                        Some(c) => end += c.len_utf8(),
                        None => break,
                    }
                }
                self.line_remainder.drain(self.line_cursor..end);
            }
            '@' => {
                let room = MAX_LINE_BYTES.saturating_sub(self.line_remainder.len());
                let add = n.min(room);
                self.line_remainder
                    .insert_str(self.line_cursor, &" ".repeat(add));
            }
            _ => {}
        }
    }

    /// Apply a runtime event to this session's state.
    fn apply_event(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::TerminalLine(e) => {
                self.ingest_terminal_chunk(&e.text);
            }
            RuntimeEvent::SessionReady(e) => {
                self.session_state = SessionState::Active;
                self.cwd = Some(e.cwd.clone());
            }
            RuntimeEvent::SessionCwdChanged(e) => {
                self.cwd = Some(e.cwd.clone());
            }
            RuntimeEvent::SessionExecStateChanged(e) => {
                self.exec_state = e.exec_state.clone();
                // runtime-core emits 'desynced' only when the shell process has
                // exited. The session is over: it cannot take input again.
                if e.exec_state == "desynced" {
                    self.session_state = SessionState::Closed;
                }
            }
            RuntimeEvent::ExecutionStarted(_) => {
                self.exec_state = "running".to_string();
            }
            RuntimeEvent::ExecutionFinished(_) => {
                self.exec_state = "ready".to_string();
            }
        }
    }
}

/// Top-level Console model — holds all sessions + app-wide state.
pub struct Model {
    /// All session models, ordered by creation.
    pub sessions: Vec<SessionModel>,
    /// Index of the active session (targets input/display).
    pub active_index: usize,

    // --- App-wide state ---
    pub input_mode: InputMode,
    pub pane_cols: u16,
    pub pane_rows: u16,
    pub should_quit: bool,

    // --- Intent / Ask mode ---
    /// Composer text (app-wide — you can only compose one intent at a time).
    pub composer_text: String,
    pub composer_cursor: usize,
    pub planner_busy: bool,
    pub planner_error: Option<String>,
    /// Bumped when an in-flight planner request is cancelled. A result carrying
    /// an older epoch is dropped when it arrives.
    pub planner_epoch: u64,

    // --- Proposal ownership (session-bound) ---
    /// The current proposal under review, if any.
    pub current_proposal: Option<CommandProposal>,
    /// The session ID that owns the current proposal.
    /// Approval executes on THIS session, not the active session.
    pub proposal_session_id: Option<String>,
    /// Vertical offset into the quoted command lines in the review pane.
    pub review_scroll: usize,
    /// Horizontal offset into those lines, in characters.
    pub review_scroll_x: usize,
    /// Command-line viewport recorded by the last review render. 0 means not yet drawn.
    pub review_rows: usize,
    /// Command-line width recorded by the last review render. 0 means not yet drawn.
    pub review_cols: usize,
    /// Explicit ack for a proposal whose `requires_confirmation` is set.
    pub proposal_confirmed: bool,
    /// Approve/cancel keys are ignored until this instant. Set when a proposal
    /// arrives from the planner; None means no debounce.
    pub review_armed_at: Option<std::time::Instant>,
    /// Execute or approval-gate error, shown in the review pane.
    pub review_error: Option<String>,
    /// One-line status when the active session rejects write, interrupt, resync, resize, or close.
    pub status_line: Option<String>,

    /// Cursor position in the run selector overlay.
    pub switcher_cursor: usize,
    /// First session row drawn by the last overlay render.
    pub switcher_start: usize,
    /// Session rows the last overlay render could show. 0 means not yet drawn.
    pub switcher_rows: usize,

    /// Why the last session create failed. Shown in the empty pane and the footer.
    pub create_error: Option<String>,

    /// Whether the help overlay is visible.
    pub show_help: bool,

    /// A destructive chord waits for y/n. Any other key cancels it.
    pub pending_confirm: Option<PendingConfirm>,
}

/// An action held back until the user presses `y`.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingConfirm {
    /// Close the session with this id; it has a running command.
    CloseSession(String),
    /// Quit; this kills every session.
    Quit,
}

pub const CLOSE_RUNNING_PROMPT: &str = "Close session with a running command? y/n";
pub const QUIT_PROMPT: &str = "Quit and end every session (running commands included)? y/n";

impl Model {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            active_index: 0,
            input_mode: InputMode::Shell,
            pane_cols: 80,
            pane_rows: 24,
            should_quit: false,
            composer_text: String::new(),
            composer_cursor: 0,
            planner_busy: false,
            planner_error: None,
            planner_epoch: 0,
            current_proposal: None,
            proposal_session_id: None,
            review_scroll: 0,
            review_scroll_x: 0,
            review_rows: 0,
            review_cols: 0,
            proposal_confirmed: false,
            review_armed_at: None,
            review_error: None,
            status_line: None,
            switcher_cursor: 0,
            switcher_start: 0,
            switcher_rows: 0,
            create_error: None,
            show_help: false,
            pending_confirm: None,
        }
    }

    /// Ask before a destructive chord. The prompt also goes to the status line.
    pub fn ask_confirm(&mut self, pending: PendingConfirm) {
        let prompt = match pending {
            PendingConfirm::CloseSession(_) => CLOSE_RUNNING_PROMPT,
            PendingConfirm::Quit => QUIT_PROMPT,
        };
        self.status_line = Some(prompt.to_string());
        self.pending_confirm = Some(pending);
    }

    /// The prompt for the pending confirmation, if any.
    pub fn confirm_prompt(&self) -> Option<&'static str> {
        self.pending_confirm.as_ref().map(|p| match p {
            PendingConfirm::CloseSession(_) => CLOSE_RUNNING_PROMPT,
            PendingConfirm::Quit => QUIT_PROMPT,
        })
    }

    /// Whether the session with this id has a running command.
    pub fn session_is_running(&self, id: &str) -> bool {
        self.sessions
            .iter()
            .any(|s| s.id == id && s.has_running_command())
    }

    /// Whether any session has a running command.
    pub fn any_session_running(&self) -> bool {
        self.sessions.iter().any(|s| s.has_running_command())
    }

    /// Add a session and return its index.
    pub fn add_session(&mut self, id: String, label: String) -> usize {
        let idx = self.sessions.len();
        self.sessions.push(SessionModel::new(id, label));
        idx
    }

    /// Get the active session, if any.
    pub fn active_session(&self) -> Option<&SessionModel> {
        self.sessions.get(self.active_index)
    }

    /// Get the active session mutably.
    pub fn active_session_mut(&mut self) -> Option<&mut SessionModel> {
        self.sessions.get_mut(self.active_index)
    }

    /// Get the active session ID.
    pub fn active_session_id(&self) -> Option<&str> {
        self.active_session().map(|s| s.id.as_str())
    }

    /// Whether the active session can accept input.
    pub fn can_accept_input(&self) -> bool {
        self.active_session().map_or(false, |s| s.can_accept_input())
    }

    /// Whether the active session is ready.
    #[allow(dead_code)]
    pub fn is_ready(&self) -> bool {
        self.active_session().map_or(false, |s| s.is_ready())
    }

    /// Switch to next session. Clears unread on newly active session.
    pub fn next_session(&mut self) {
        if !self.sessions.is_empty() {
            self.active_index = (self.active_index + 1) % self.sessions.len();
            self.sessions[self.active_index].has_unread = false;
        }
    }

    /// Switch to previous session. Clears unread on newly active session.
    pub fn prev_session(&mut self) {
        if !self.sessions.is_empty() {
            self.active_index = if self.active_index == 0 {
                self.sessions.len() - 1
            } else {
                self.active_index - 1
            };
            self.sessions[self.active_index].has_unread = false;
        }
    }

    /// Switch to session by index. Clears unread on newly active session.
    pub fn switch_to(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active_index = index;
            self.sessions[index].has_unread = false;
        }
    }

    /// Clear the unread dot on the session `active_index` names. The user is
    /// looking at it, so it is not unread.
    pub fn clear_active_unread(&mut self) {
        if let Some(s) = self.sessions.get_mut(self.active_index) {
            s.has_unread = false;
        }
    }

    /// Abort the in-flight planner request. Its result is dropped on arrival.
    pub fn cancel_planner(&mut self) {
        self.planner_epoch = self.planner_epoch.wrapping_add(1);
        self.planner_busy = false;
        self.planner_error = None;
    }

    /// Open the run selector overlay.
    pub fn open_switcher(&mut self) {
        self.clear_active_unread();
        self.switcher_cursor = self.active_index;
        self.switcher_start = 0;
        self.switcher_rows = 0;
        self.input_mode = InputMode::Switcher;
    }

    /// Close the run selector overlay without switching.
    pub fn close_switcher(&mut self) {
        self.input_mode = InputMode::Shell;
    }

    /// Confirm selection in the run selector — switch to cursor position.
    pub fn confirm_switcher(&mut self) {
        self.switch_to(self.switcher_cursor);
        self.input_mode = InputMode::Shell;
    }

    // --- Proposal ownership law ---

    /// Set a proposal with explicit session binding.
    /// Scroll and confirmation start over so the new command is reviewed from the top.
    pub fn set_proposal(&mut self, proposal: CommandProposal, session_id: String) {
        self.current_proposal = Some(proposal);
        self.proposal_session_id = Some(session_id);
        self.review_scroll = 0;
        self.review_scroll_x = 0;
        self.proposal_confirmed = false;
        self.review_error = None;
    }

    /// Debounce window after a proposal lands: approve keys pressed while the
    /// planner was still running must not execute an unseen command.
    pub const REVIEW_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(600);

    /// Start the approve debounce for a freshly arrived proposal.
    pub fn arm_review_debounce(&mut self) {
        self.review_armed_at = Some(std::time::Instant::now() + Self::REVIEW_DEBOUNCE);
    }

    /// True once the debounce window (if any) has passed.
    pub fn review_input_ready(&self) -> bool {
        self.review_armed_at
            .is_none_or(|t| std::time::Instant::now() >= t)
    }

    /// If the owning session's cwd is known and differs from the cwd the
    /// proposal was written for, say so. None means safe to approve.
    pub fn cwd_drift(&self, proposal: &CommandProposal, session_id: &str) -> Option<String> {
        let planned = proposal.cwd.as_deref().filter(|c| !c.is_empty() && *c != ".")?;
        let now = self
            .sessions
            .iter()
            .find(|s| s.id == session_id)?
            .cwd
            .as_deref()?;
        if now == planned {
            None
        } else {
            Some(format!(
                "Directory changed since this was planned ({planned} -> {now}) — cancel and ask again"
            ))
        }
    }

    /// Remove a session by id, keeping the active session selected.
    /// Returns false when no such session exists.
    pub fn remove_session_by_id(&mut self, id: &str) -> bool {
        let Some(idx) = self.sessions.iter().position(|s| s.id == id) else {
            return false;
        };
        let active_id = self.sessions.get(self.active_index).map(|s| s.id.clone());
        self.sessions.remove(idx);
        if let Some(aid) = active_id.filter(|a| a != id) {
            if let Some(i) = self.sessions.iter().position(|s| s.id == aid) {
                self.active_index = i;
            }
        } else if self.active_index >= self.sessions.len() {
            self.active_index = self.sessions.len().saturating_sub(1);
        }
        self.clear_active_unread();
        true
    }

    /// Get the session ID that owns the current proposal.
    pub fn proposal_owner(&self) -> Option<&str> {
        self.proposal_session_id.as_deref()
    }

    /// Clear the current proposal (cancel or after a successful execution).
    pub fn clear_proposal(&mut self) {
        self.current_proposal = None;
        self.proposal_session_id = None;
        self.review_scroll = 0;
        self.review_scroll_x = 0;
        self.proposal_confirmed = false;
        self.review_armed_at = None;
        self.review_error = None;
    }

    /// Record a session write, interrupt, resync, resize, or close result.
    /// Ok clears a stale status. Err replaces it with the error string.
    pub fn surface_session_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.status_line = None,
            Err(err) => self.status_line = Some(err),
        }
    }

    /// Whether a proposal exists and its owning session still exists.
    pub fn has_valid_proposal(&self) -> bool {
        if let Some(ref sid) = self.proposal_session_id {
            self.sessions.iter().any(|s| s.id == *sid)
        } else {
            false
        }
    }

    /// Whether switching sessions is allowed right now.
    /// Blocked during Review and RawPlay — must exit those modes first.
    pub fn can_switch_sessions(&self) -> bool {
        !matches!(self.input_mode, InputMode::Review | InputMode::RawPlay)
    }

    /// Whether the active session is still alive (exists and not Closed/Error).
    pub fn active_session_alive(&self) -> bool {
        self.active_session().map_or(false, |s| {
            matches!(s.session_state, SessionState::Booting | SessionState::Active)
        })
    }

    /// Find session index by ID.
    pub fn session_index(&self, session_id: &str) -> Option<usize> {
        self.sessions.iter().position(|s| s.id == session_id)
    }

    /// Number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Route a runtime event to the correct session by session_id.
    /// This is the targeting truth — events ONLY update their own session.
    /// Sets has_unread on non-active sessions that receive output.
    pub fn apply_event(&mut self, event: RuntimeEvent) {
        let session_id = match &event {
            RuntimeEvent::TerminalLine(e) => &e.session_id,
            RuntimeEvent::SessionReady(e) => &e.session_id,
            RuntimeEvent::SessionCwdChanged(e) => &e.session_id,
            RuntimeEvent::SessionExecStateChanged(e) => &e.session_id,
            RuntimeEvent::ExecutionStarted(e) => &e.execution.session_id,
            RuntimeEvent::ExecutionFinished(e) => &e.session_id,
        };

        let is_output = matches!(&event, RuntimeEvent::TerminalLine(_));
        let active_id = self.active_session_id().map(|s| s.to_string());

        if let Some(idx) = self.sessions.iter().position(|s| s.id == *session_id) {
            self.sessions[idx].apply_event(&event);

            if matches!(&event, RuntimeEvent::SessionExecStateChanged(e) if e.exec_state == "desynced")
            {
                self.status_line = Some(format!(
                    "{} exited — Ctrl+W closes it",
                    self.sessions[idx].label
                ));
            }

            // Mark unread if output arrived on a non-active session
            if is_output && active_id.as_deref() != Some(session_id) {
                self.sessions[idx].has_unread = true;
            }
        }
    }

    // --- Composer methods (unchanged, app-wide) ---

    pub fn composer_insert(&mut self, c: char) {
        self.composer_text.insert(self.composer_cursor, c);
        self.composer_cursor += c.len_utf8();
    }

    pub fn composer_backspace(&mut self) {
        if self.composer_cursor > 0 {
            let prev = self.composer_text[..self.composer_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.composer_text.drain(prev..self.composer_cursor);
            self.composer_cursor = prev;
        }
    }

    pub fn composer_left(&mut self) {
        if self.composer_cursor > 0 {
            self.composer_cursor = self.composer_text[..self.composer_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn composer_right(&mut self) {
        if self.composer_cursor < self.composer_text.len() {
            self.composer_cursor = self.composer_text[self.composer_cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.composer_cursor + i)
                .unwrap_or(self.composer_text.len());
        }
    }

    pub fn composer_clear(&mut self) {
        self.composer_text.clear();
        self.composer_cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commandui_runtime_core::events::*;

    fn make_line_event(session_id: &str, text: &str) -> RuntimeEvent {
        RuntimeEvent::TerminalLine(TerminalLineEvent {
            id: "l1".to_string(),
            session_id: session_id.to_string(),
            execution_id: None,
            kind: "stdout".to_string(),
            text: text.to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        })
    }

    fn make_ready_event(session_id: &str, cwd: &str) -> RuntimeEvent {
        RuntimeEvent::SessionReady(SessionReadyEvent {
            session_id: session_id.to_string(),
            cwd: cwd.to_string(),
        })
    }

    #[test]
    fn test_add_session() {
        let mut model = Model::new();
        let idx = model.add_session("s1".into(), "Session 1".into());
        assert_eq!(idx, 0);
        assert_eq!(model.session_count(), 1);
        assert_eq!(model.active_session_id(), Some("s1"));
    }

    #[test]
    fn test_multi_session_switching() {
        let mut model = Model::new();
        model.add_session("s1".into(), "Session 1".into());
        model.add_session("s2".into(), "Session 2".into());
        model.add_session("s3".into(), "Session 3".into());

        assert_eq!(model.active_session_id(), Some("s1"));

        model.next_session();
        assert_eq!(model.active_session_id(), Some("s2"));

        model.next_session();
        assert_eq!(model.active_session_id(), Some("s3"));

        // Wraps around
        model.next_session();
        assert_eq!(model.active_session_id(), Some("s1"));

        model.prev_session();
        assert_eq!(model.active_session_id(), Some("s3"));
    }

    #[test]
    fn test_switch_to_index() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.switch_to(1);
        assert_eq!(model.active_session_id(), Some("s2"));

        // Out of range does nothing
        model.switch_to(99);
        assert_eq!(model.active_session_id(), Some("s2"));
    }

    #[test]
    fn test_event_routing_targets_correct_session() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        // Event for s1 should only affect s1
        model.apply_event(make_line_event("s1", "hello from s1\n"));

        assert_eq!(model.sessions[0].terminal_lines, vec!["hello from s1"]);
        assert!(model.sessions[1].terminal_lines.is_empty());

        // Event for s2 should only affect s2
        model.apply_event(make_line_event("s2", "hello from s2\n"));

        assert_eq!(model.sessions[0].terminal_lines, vec!["hello from s1"]);
        assert_eq!(model.sessions[1].terminal_lines, vec!["hello from s2"]);
    }

    #[test]
    fn test_event_for_unknown_session_is_ignored() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        // Event for nonexistent session — should not panic or affect anything
        model.apply_event(make_line_event("unknown", "ghost\n"));

        assert!(model.sessions[0].terminal_lines.is_empty());
    }

    #[test]
    fn test_session_ready_targets_correct_session() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.apply_event(make_ready_event("s2", "/home/user"));

        assert_eq!(model.sessions[0].session_state, SessionState::Booting);
        assert_eq!(model.sessions[1].session_state, SessionState::Active);
        assert_eq!(model.sessions[1].cwd.as_deref(), Some("/home/user"));
    }

    #[test]
    fn test_active_session_input_gating() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        // Booting session should not accept input
        assert!(!model.can_accept_input());

        model.apply_event(make_ready_event("s1", "/tmp"));
        assert!(model.can_accept_input());
    }

    #[test]
    fn test_per_session_scroll_isolation() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        // Add lines to s1
        for i in 0..50 {
            model.apply_event(make_line_event("s1", &format!("line {i}\n")));
        }

        // Scroll s1 (via active session)
        model.sessions[0].scroll_up(10);
        assert_eq!(model.sessions[0].scroll_offset, 10);

        // s2 scroll should be independent
        assert_eq!(model.sessions[1].scroll_offset, 0);
    }

    #[test]
    fn test_per_session_buffer_cap() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        for i in 0..10_050 {
            model.sessions[0].terminal_lines.push(format!("line {i}"));
        }
        model.apply_event(make_line_event("s1", "overflow\n"));
        assert!(model.sessions[0].terminal_lines.len() <= 10_000);
    }

    #[test]
    fn test_composer_insert_and_backspace() {
        let mut model = Model::new();
        model.composer_insert('h');
        model.composer_insert('i');
        assert_eq!(model.composer_text, "hi");
        model.composer_backspace();
        assert_eq!(model.composer_text, "h");
    }

    #[test]
    fn test_composer_clear() {
        let mut model = Model::new();
        model.composer_text = "intent".to_string();
        model.composer_cursor = 3;
        model.composer_clear();
        assert_eq!(model.composer_text, "");
        assert_eq!(model.composer_cursor, 0);
    }

    #[test]
    fn test_default_input_mode_is_shell() {
        let model = Model::new();
        assert_eq!(model.input_mode, InputMode::Shell);
    }

    #[test]
    fn test_session_index_lookup() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        assert_eq!(model.session_index("s1"), Some(0));
        assert_eq!(model.session_index("s2"), Some(1));
        assert_eq!(model.session_index("unknown"), None);
    }

    #[test]
    fn test_unread_set_on_non_active_output() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());
        // s1 is active (index 0)

        // Output for s2 (non-active) should set has_unread
        model.apply_event(make_line_event("s2", "background output\n"));
        assert!(model.sessions[1].has_unread);

        // Output for s1 (active) should NOT set has_unread
        assert!(!model.sessions[0].has_unread);
    }

    #[test]
    fn test_unread_cleared_on_switch() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.apply_event(make_line_event("s2", "output\n"));
        assert!(model.sessions[1].has_unread);

        // Switching to s2 should clear its unread flag
        model.next_session();
        assert!(!model.sessions[1].has_unread);
    }

    #[test]
    fn test_state_badge_values() {
        let mut session = SessionModel::new("s1".into(), "A".into());
        assert_eq!(session.state_badge(), "BOOT");

        session.session_state = SessionState::Active;
        session.exec_state = "ready".to_string();
        assert_eq!(session.state_badge(), "IDLE");

        session.exec_state = "running".to_string();
        assert_eq!(session.state_badge(), "RUNNING");

        session.exec_state = "interrupting".to_string();
        assert_eq!(session.state_badge(), "STOPPING");

        session.session_state = SessionState::Closed;
        assert_eq!(session.state_badge(), "DONE");

        session.session_state = SessionState::Error("oops".into());
        assert_eq!(session.state_badge(), "ERROR");
    }

    #[test]
    fn test_switcher_lifecycle() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());
        model.add_session("s3".into(), "C".into());

        // Open switcher — cursor starts at active index
        model.open_switcher();
        assert_eq!(model.input_mode, InputMode::Switcher);
        assert_eq!(model.switcher_cursor, 0);

        // Move cursor
        model.switcher_cursor = 2;

        // Confirm — switches to cursor position and closes
        model.confirm_switcher();
        assert_eq!(model.input_mode, InputMode::Shell);
        assert_eq!(model.active_index, 2);
        assert_eq!(model.active_session_id(), Some("s3"));
    }

    #[test]
    fn test_switcher_close_without_switch() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.open_switcher();
        model.switcher_cursor = 1; // Cursor on s2

        // Close without confirming — stays on s1
        model.close_switcher();
        assert_eq!(model.input_mode, InputMode::Shell);
        assert_eq!(model.active_index, 0);
    }

    // --- Proposal ownership / targeting integrity tests ---

    fn make_proposal(session_id: &str) -> CommandProposal {
        commandui_runtime_planner::CommandProposal {
            id: "p1".to_string(),
            session_id: session_id.to_string(),
            source: "mock".to_string(),
            user_intent: "test".to_string(),
            command: "echo test".to_string(),
            cwd: Some("/tmp".to_string()),
            explanation: "Test command".to_string(),
            assumptions: vec![],
            confidence: 0.95,
            risk: "low".to_string(),
            destructive: false,
            requires_confirmation: false,
            touches_files: false,
            touches_network: false,
            escalates_privileges: false,
            expected_output: None,
            generated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_proposal_session_binding() {
        let mut model = Model::new();
        model.add_session("s1".into(), "Run A".into());
        model.add_session("s2".into(), "Run B".into());

        // Set proposal bound to s1
        model.set_proposal(make_proposal("s1"), "s1".into());

        assert_eq!(model.proposal_owner(), Some("s1"));
        assert!(model.has_valid_proposal());
    }

    #[test]
    fn test_proposal_survives_active_session_change() {
        let mut model = Model::new();
        model.add_session("s1".into(), "Run A".into());
        model.add_session("s2".into(), "Run B".into());

        // Proposal bound to s1, active is s1
        model.set_proposal(make_proposal("s1"), "s1".into());
        model.input_mode = InputMode::Review;

        // Proposal owner is still s1 regardless of model state
        assert_eq!(model.proposal_owner(), Some("s1"));

        // Even if we force-change active index (shouldn't happen in review, but test the data)
        model.active_index = 1;
        assert_eq!(model.proposal_owner(), Some("s1")); // Still s1!
    }

    #[test]
    fn test_clear_proposal_resets_both_fields() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        model.set_proposal(make_proposal("s1"), "s1".into());
        assert!(model.current_proposal.is_some());
        assert!(model.proposal_session_id.is_some());

        model.review_scroll = 3;
        model.proposal_confirmed = true;
        model.review_error = Some("old".into());
        model.clear_proposal();
        assert!(model.current_proposal.is_none());
        assert!(model.proposal_session_id.is_none());
        assert_eq!(model.review_scroll, 0);
        assert!(!model.proposal_confirmed);
        assert!(model.review_error.is_none());
    }

    #[test]
    fn set_proposal_resets_the_review_gate() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.review_scroll = 4;
        model.review_scroll_x = 2;
        model.proposal_confirmed = true;
        model.review_error = Some("old".into());

        model.set_proposal(make_proposal("s1"), "s1".into());

        assert_eq!(model.review_scroll, 0);
        assert_eq!(model.review_scroll_x, 0);
        assert!(!model.proposal_confirmed);
        assert!(model.review_error.is_none());
        assert!(model.current_proposal.is_some());
    }

    #[test]
    fn test_proposal_invalid_if_session_removed() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.set_proposal(make_proposal("s1"), "s1".into());
        assert!(model.has_valid_proposal());

        // Remove s1
        model.sessions.remove(0);
        model.active_index = 0; // now points to s2

        // Proposal is no longer valid — owning session is gone
        assert!(!model.has_valid_proposal());
    }

    #[test]
    fn test_switching_blocked_during_review() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.input_mode = InputMode::Review;
        assert!(!model.can_switch_sessions());

        model.input_mode = InputMode::Shell;
        assert!(model.can_switch_sessions());

        model.input_mode = InputMode::Ask;
        assert!(model.can_switch_sessions());
    }

    #[test]
    fn test_output_during_review_does_not_corrupt_proposal() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        // Proposal for s1, in review
        model.set_proposal(make_proposal("s1"), "s1".into());
        model.input_mode = InputMode::Review;

        // Output arrives on s2 while reviewing s1's proposal
        model.apply_event(make_line_event("s2", "background noise\n"));

        // Proposal is untouched
        assert_eq!(model.proposal_owner(), Some("s1"));
        assert!(model.current_proposal.is_some());
        assert_eq!(model.current_proposal.as_ref().unwrap().command, "echo test");

        // s2 got its output
        assert_eq!(model.sessions[1].terminal_lines, vec!["background noise"]);
    }

    // --- Raw play mode tests ---

    #[test]
    fn test_switching_blocked_during_raw_play() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        model.input_mode = InputMode::RawPlay;
        assert!(!model.can_switch_sessions());

        model.input_mode = InputMode::Shell;
        assert!(model.can_switch_sessions());
    }

    #[test]
    fn test_active_session_alive_tracks_lifecycle() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        // Booting is alive
        assert!(model.active_session_alive());

        // Active is alive
        model.sessions[0].session_state = SessionState::Active;
        assert!(model.active_session_alive());

        // Closed is not alive
        model.sessions[0].session_state = SessionState::Closed;
        assert!(!model.active_session_alive());

        // Error is not alive
        model.sessions[0].session_state = SessionState::Error("oops".into());
        assert!(!model.active_session_alive());
    }

    #[test]
    fn test_raw_play_does_not_affect_other_sessions_unread() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());

        // Enter raw play on s1 (active)
        model.input_mode = InputMode::RawPlay;

        // Output for s2 while s1 is in raw play — should still mark s2 unread
        model.apply_event(make_line_event("s2", "background\n"));
        assert!(model.sessions[1].has_unread);

        // Output for s1 — active session, should NOT be marked unread
        model.apply_event(make_line_event("s1", "game output\n"));
        assert!(!model.sessions[0].has_unread);
    }

    #[test]
    fn test_raw_play_output_still_captured_in_model() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.input_mode = InputMode::RawPlay;

        // Output during raw play should still be captured in the model
        // (even though it's also being written to stdout by the app layer)
        model.apply_event(make_line_event("s1", "game frame data\n"));
        assert_eq!(model.sessions[0].terminal_lines, vec!["game frame data"]);
    }

    #[test]
    fn test_no_sessions_means_not_alive() {
        let model = Model::new();
        assert!(!model.active_session_alive());
    }

    #[test]
    fn test_help_overlay_default_hidden() {
        let model = Model::new();
        assert!(!model.show_help);
    }

    #[test]
    fn test_help_overlay_toggle() {
        let mut model = Model::new();
        model.show_help = true;
        assert!(model.show_help);
        model.show_help = false;
        assert!(!model.show_help);
    }

    #[test]
    fn test_welcome_banner_shows_when_no_sessions() {
        let model = Model::new();
        assert!(model.sessions.is_empty());
        // Welcome banner is rendered when sessions is empty
        // (verified by ui.rs rendering path — model truth is sessions.is_empty())
    }

    #[test]
    fn test_welcome_banner_gone_after_session_added() {
        let mut model = Model::new();
        model.add_session("s1".into(), "Session 1".into());
        assert!(!model.sessions.is_empty());
    }

    #[test]
    fn test_split_write_joins_remainder() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        model.apply_event(make_line_event("s1", "hel"));
        assert!(model.sessions[0].terminal_lines.is_empty());
        assert_eq!(model.sessions[0].line_remainder, "hel");

        model.apply_event(make_line_event("s1", "lo\n"));
        assert_eq!(model.sessions[0].terminal_lines, vec!["hello"]);
        assert_eq!(model.sessions[0].line_remainder, "");
    }

    #[test]
    fn test_blank_line_is_preserved() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        model.apply_event(make_line_event("s1", "top\n\nbottom\n"));
        assert_eq!(
            model.sessions[0].terminal_lines,
            vec!["top".to_string(), String::new(), "bottom".to_string()]
        );
    }

    #[test]
    fn test_carriage_return_overwrites_current_line() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        model.apply_event(make_line_event("s1", "abc"));
        model.apply_event(make_line_event("s1", "\rXY\n"));
        assert_eq!(model.sessions[0].terminal_lines, vec!["XY"]);

        // CR LF is a newline, not an erased line.
        model.apply_event(make_line_event("s1", "hello\r\n"));
        assert_eq!(model.sessions[0].terminal_lines, vec!["XY", "hello"]);
    }

    fn feed(model: &mut Model, chunks: &[&str]) {
        for chunk in chunks {
            model.apply_event(make_line_event("s1", chunk));
        }
    }

    #[test]
    fn backspace_and_del_erase_the_previous_character() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        // Readline erase of the 'c' in "abc", then "d".
        feed(&mut model, &["abc\u{8} \u{8}d\n"]);
        assert_eq!(model.sessions[0].terminal_lines, vec!["abd"]);
        feed(&mut model, &["xyz\u{7f}\u{7f}Q\n"]);
        assert_eq!(model.sessions[0].terminal_lines, vec!["abd", "xQ"]);
        // Erase on an empty line is a no-op, and CR LF still ends the line.
        feed(&mut model, &["\u{8}\u{7f}ok\r\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "ok");
        assert!(model.sessions[0]
            .terminal_lines
            .iter()
            .all(|l| !l.chars().any(|c| c.is_control())));
    }

    #[test]
    fn ansi_sequences_are_stripped_before_the_line_is_stored() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        feed(
            &mut model,
            &[
                "\u{1b}[01;34mdir\u{1b}[0m  \u{1b}[1;31;4mfile\u{1b}[m\n",
                "\u{1b}]0;window title\u{7}after-bel\n",
                "\u{1b}]8;;http://x\u{1b}\\link\u{1b}]8;;\u{1b}\\\n",
                "\u{1b}(Bplain\u{1b}[?25l\u{1b}[2K!\n",
            ],
        );
        assert_eq!(
            model.sessions[0].terminal_lines,
            // CSI 2K erases the whole line, so "plain" is gone before the "!".
            vec!["dir  file", "after-bel", "link", "!"]
        );
    }

    #[test]
    fn an_escape_sequence_split_across_chunks_is_still_stripped() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        feed(&mut model, &["a\u{1b}", "[01;3", "4mb\u{1b}]0;ti", "tle\u{7}c\n"]);
        assert_eq!(model.sessions[0].terminal_lines, vec!["abc"]);
        assert_eq!(model.sessions[0].line_remainder, "");
    }

    #[test]
    fn a_runaway_osc_does_not_swallow_following_lines() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        feed(&mut model, &["\u{1b}]0;never ended\nnext\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "next");
    }

    #[test]
    fn a_newline_free_stream_is_capped_by_bytes_and_counted() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        let chunk = "x".repeat(10_000);
        for _ in 0..10 {
            feed(&mut model, &[&chunk]);
        }
        let s = &model.sessions[0];
        assert!(s.line_remainder.len() <= MAX_LINE_BYTES);
        assert_eq!(s.line_remainder.len() + s.bytes_dropped, 100_000);
        feed(&mut model, &["\n"]);
        assert!(model.sessions[0].terminal_lines[0].len() <= MAX_LINE_BYTES);
        // A multibyte character is never split at the cap.
        feed(&mut model, &[&"\u{65e5}".repeat(10_000)]);
        assert!(model.sessions[0].line_remainder.len() <= MAX_LINE_BYTES);
        assert!(model.sessions[0].line_remainder.chars().all(|c| c == '\u{65e5}'));
    }

    #[test]
    fn a_scrolled_up_viewport_stays_on_the_same_lines_while_output_arrives() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        for i in 0..50 {
            feed(&mut model, &[&format!("line {i}\n")]);
        }
        model.sessions[0].scroll_up(10);
        assert_eq!(model.sessions[0].scroll_offset, 10);
        let anchored = |m: &Model| {
            let s = &m.sessions[0];
            s.terminal_lines[s.terminal_lines.len() - s.scroll_offset - 1].clone()
        };
        let before = anchored(&model);
        feed(&mut model, &["a\nb\nc\n"]);
        assert_eq!(model.sessions[0].scroll_offset, 13);
        assert_eq!(anchored(&model), before);

        // At the live end the offset stays 0 and follows the output.
        model.sessions[0].scroll_to_bottom();
        feed(&mut model, &["d\n"]);
        assert_eq!(model.sessions[0].scroll_offset, 0);
    }

    #[test]
    fn a_front_trim_keeps_the_same_lines_and_counts_the_drop() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        for i in 0..MAX_LINES {
            model.sessions[0].terminal_lines.push(format!("line {i}"));
        }
        model.sessions[0].scroll_offset = 100;
        let target = model.sessions[0].terminal_lines[MAX_LINES - 101].clone();
        feed(&mut model, &["one\ntwo\n"]);
        let s = &model.sessions[0];
        assert_eq!(s.terminal_lines.len(), MAX_LINES);
        assert_eq!(s.lines_dropped, 2);
        // Two lines arrived while scrolled up: +2, then the trim leaves it alone.
        assert_eq!(s.scroll_offset, 102);
        assert_eq!(s.terminal_lines[MAX_LINES - s.scroll_offset - 1], target);

        // An offset deeper than what is left is clamped, not moved to the live end.
        model.sessions[0].scroll_offset = MAX_LINES + 50;
        feed(&mut model, &["x\n"]);
        assert_eq!(model.sessions[0].scroll_offset, MAX_LINES - 1);
    }

    #[test]
    fn unread_is_cleared_on_the_session_the_active_index_names() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.add_session("s2".into(), "B".into());
        model.sessions[1].has_unread = true;
        model.active_index = 1;
        model.clear_active_unread();
        assert!(!model.sessions[1].has_unread);
        model.sessions[0].has_unread = true;
        model.open_switcher();
        assert!(model.sessions[0].has_unread, "only the active row is cleared");
    }

    #[test]
    fn runtime_events_update_cwd_and_exec_state() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());

        model.apply_event(RuntimeEvent::SessionCwdChanged(SessionCwdChangedEvent {
            session_id: "s1".into(),
            cwd: "/work".into(),
        }));
        assert_eq!(model.sessions[0].cwd.as_deref(), Some("/work"));

        model.apply_event(RuntimeEvent::SessionExecStateChanged(
            SessionExecStateChangedEvent {
                session_id: "s1".into(),
                exec_state: "desynced".into(),
                changed_at: "t".into(),
            },
        ));
        assert_eq!(model.sessions[0].exec_state, "desynced");

        model.apply_event(RuntimeEvent::ExecutionStarted(ExecutionStartedEvent {
            execution: ExecutionSummary {
                id: "e1".into(),
                session_id: "s1".into(),
                command: "echo hi".into(),
                source: "ask".into(),
                linked_plan_id: None,
                status: "running".into(),
                started_at: "t".into(),
                finished_at: None,
                exit_code: None,
            },
        }));
        assert_eq!(model.sessions[0].exec_state, "running");

        model.apply_event(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
            execution_id: "e1".into(),
            session_id: "s1".into(),
            exit_code: 0,
            finished_at: "t".into(),
            status: "success".into(),
        }));
        assert_eq!(model.sessions[0].exec_state, "ready");

        model.apply_event(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
            execution_id: "e2".into(),
            session_id: "missing".into(),
            exit_code: 1,
            finished_at: "t".into(),
            status: "failure".into(),
        }));
        assert_eq!(model.sessions[0].exec_state, "ready");
    }

    #[test]
    fn scroll_composer_and_empty_model_edges() {
        let mut session = SessionModel::new("s".into(), "A".into());
        assert!(!session.is_ready());
        session.terminal_lines = vec!["a".into(), "b".into(), "c".into()];
        session.scroll_up(100);
        assert_eq!(session.scroll_offset, 2);
        session.scroll_down(1);
        assert_eq!(session.scroll_offset, 1);
        session.scroll_to_bottom();
        assert_eq!(session.scroll_offset, 0);
        session.session_state = SessionState::Active;
        assert!(session.is_ready());

        let mut model = Model::new();
        assert!(!model.is_ready());
        assert!(!model.can_accept_input());
        assert!(!model.has_valid_proposal());
        model.next_session();
        model.prev_session();
        assert!(model.sessions.is_empty());
        model.surface_session_result(Err("nope".into()));
        assert_eq!(model.status_line.as_deref(), Some("nope"));
        model.surface_session_result(Ok(()));
        assert!(model.status_line.is_none());

        model.add_session("s1".into(), "A".into());
        model.active_index = 9;
        assert!(model.active_session().is_none());
        assert!(model.active_session_mut().is_none());
        assert!(!model.is_ready());
        assert!(!model.can_accept_input());
        assert!(!model.active_session_alive());
        model.active_index = 0;
        model.sessions[0].session_state = SessionState::Active;
        assert!(model.is_ready());

        model.composer_insert('你');
        model.composer_insert('a');
        assert_eq!(model.composer_text, "你a");
        model.composer_left();
        assert_eq!(model.composer_cursor, "你".len());
        model.composer_left();
        assert_eq!(model.composer_cursor, 0);
        model.composer_left();
        assert_eq!(model.composer_cursor, 0);
        model.composer_right();
        assert_eq!(model.composer_cursor, "你".len());
        model.composer_insert('b');
        assert_eq!(model.composer_text, "你ba");
        model.composer_right();
        model.composer_right();
        assert_eq!(model.composer_cursor, model.composer_text.len());
        model.composer_backspace();
        model.composer_backspace();
        model.composer_backspace();
        assert_eq!(model.composer_text, "");
        model.composer_backspace();
        assert_eq!(model.composer_cursor, 0);

        model.prev_session();
        assert_eq!(model.active_index, 0);
    }

    #[test]
    fn a_mid_line_edit_leaves_the_line_the_shell_holds() {
        // bash: type abcd, Left twice (two BS), type X (prints Xcd, two BS), Enter.
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        feed(&mut model, &["abcd\u{8}\u{8}"]);
        assert_eq!(model.sessions[0].line_remainder, "abcd");
        feed(&mut model, &["Xcd\u{8}\u{8}"]);
        assert_eq!(model.sessions[0].line_remainder, "abXcd");
        feed(&mut model, &["\r\n"]);
        assert_eq!(model.sessions[0].terminal_lines, vec!["abXcd"]);
    }

    #[test]
    fn erase_and_cursor_sequences_edit_the_open_line() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        // CSI K from the cursor, after CSI D moves it left.
        feed(&mut model, &["hello world\u{1b}[6D\u{1b}[K\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "hello");
        // A redraw: CR, new text, erase to end (PSReadLine / history recall).
        feed(&mut model, &["PS> old command\rPS> new\u{1b}[K\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "PS> new");
        // CSI P deletes in place, CSI G jumps to a column, CSI @ inserts.
        feed(&mut model, &["abcdef\u{1b}[4G\u{1b}[2P\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "abcf");
        feed(&mut model, &["abc\u{1b}[2D\u{1b}[2@\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "a  bc");
        // Colour and private-mode sequences still draw nothing.
        feed(&mut model, &["\u{1b}[?25l\u{1b}[31mred\u{1b}[0m\n"]);
        assert_eq!(model.sessions[0].terminal_lines.last().unwrap(), "red");
    }

    #[test]
    fn a_shell_exit_marks_the_session_closed_and_done() {
        let mut model = Model::new();
        model.add_session("s1".into(), "A".into());
        model.apply_event(make_ready_event("s1", "/w"));
        assert!(model.active_session_alive());
        assert_eq!(model.sessions[0].state_badge(), "IDLE");

        model.apply_event(RuntimeEvent::SessionExecStateChanged(
            SessionExecStateChangedEvent {
                session_id: "s1".into(),
                exec_state: "desynced".into(),
                changed_at: "t".into(),
            },
        ));
        assert_eq!(model.sessions[0].session_state, SessionState::Closed);
        assert_eq!(model.sessions[0].state_badge(), "DONE");
        // Raw Play auto-exits and Shell stops forwarding on this signal.
        assert!(!model.active_session_alive());
        assert!(!model.can_accept_input());
        assert!(model.status_line.as_deref().unwrap().contains("exited"));
        // A later ExecutionFinished does not reopen it.
        model.apply_event(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
            execution_id: "e".into(),
            session_id: "s1".into(),
            exit_code: 1,
            finished_at: "t".into(),
            status: "failure".into(),
        }));
        assert_eq!(model.sessions[0].session_state, SessionState::Closed);
    }
}
