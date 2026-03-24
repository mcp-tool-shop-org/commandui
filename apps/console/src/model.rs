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
    pub exec_state: String,
    pub session_state: SessionState,
    pub scroll_offset: usize,
    pub lines_dropped: usize,
    /// True when this session received output while not active.
    /// Cleared when session becomes the active session.
    pub has_unread: bool,
}

impl SessionModel {
    pub fn new(id: String, label: String) -> Self {
        Self {
            id,
            label,
            terminal_lines: Vec::new(),
            cwd: None,
            exec_state: "booting".to_string(),
            session_state: SessionState::Booting,
            scroll_offset: 0,
            lines_dropped: 0,
            has_unread: false,
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

    /// Apply a runtime event to this session's state.
    fn apply_event(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::TerminalLine(e) => {
                for line in e.text.split('\n') {
                    if !line.is_empty() {
                        self.terminal_lines.push(line.to_string());
                    }
                }
                if self.terminal_lines.len() > MAX_LINES {
                    let excess = self.terminal_lines.len() - MAX_LINES;
                    self.terminal_lines.drain(..excess);
                    self.lines_dropped += excess;
                    self.scroll_offset = self.scroll_offset.saturating_sub(excess);
                }
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

    // --- Proposal ownership (session-bound) ---
    /// The current proposal under review, if any.
    pub current_proposal: Option<CommandProposal>,
    /// The session ID that owns the current proposal.
    /// Approval executes on THIS session, not the active session.
    pub proposal_session_id: Option<String>,

    /// Cursor position in the run selector overlay.
    pub switcher_cursor: usize,

    /// Whether the help overlay is visible.
    pub show_help: bool,
}

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
            current_proposal: None,
            proposal_session_id: None,
            switcher_cursor: 0,
            show_help: false,
        }
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

    /// Open the run selector overlay.
    pub fn open_switcher(&mut self) {
        self.switcher_cursor = self.active_index;
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
    pub fn set_proposal(&mut self, proposal: CommandProposal, session_id: String) {
        self.current_proposal = Some(proposal);
        self.proposal_session_id = Some(session_id);
    }

    /// Get the session ID that owns the current proposal.
    pub fn proposal_owner(&self) -> Option<&str> {
        self.proposal_session_id.as_deref()
    }

    /// Clear the current proposal (cancel or after execution).
    pub fn clear_proposal(&mut self) {
        self.current_proposal = None;
        self.proposal_session_id = None;
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

        model.clear_proposal();
        assert!(model.current_proposal.is_none());
        assert!(model.proposal_session_id.is_none());
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
}
