//! Console app — multi-session event loop.
//!
//! Session targeting law:
//!   - Input always targets the active session
//!   - Runtime events are routed to the correct session by session_id
//!   - Interrupt/resync target the active session
//!   - Ask/Review/Approve execute on the active session
//!
//! Resize law: host resize → chrome reflow → PTY resized for active session.

use crate::input::{self, InputAction};
use crate::model::{InputMode, Model};
use crate::planner::{self, OllamaConfig};
use crate::ui;
use commandui_runtime_core::events::RuntimeEvent;
use commandui_runtime_core::services::session_service::{CreateSessionRequest, SessionService};
use commandui_runtime_core::services::terminal_service::{ExecuteRequest, TerminalService};
use crossterm::event::{self as ct_event, Event, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

#[allow(dead_code)]
enum PlannerResult {
    /// (proposal, owning_session_id) — proposal is bound to the session that asked.
    /// The epoch is the model's `planner_epoch` when the request started.
    Success((commandui_runtime_planner::CommandProposal, String), u64),
    Error(String, u64),
}

pub struct App {
    session_service: SessionService,
    terminal_service: TerminalService,
    runtime_rx: UnboundedReceiver<RuntimeEvent>,
    planner_rx: tokio::sync::mpsc::UnboundedReceiver<PlannerResult>,
    planner_tx: tokio::sync::mpsc::UnboundedSender<PlannerResult>,
    ollama_config: Arc<OllamaConfig>,
    model: Model,
    session_counter: usize,
    /// Whether the terminal is currently in raw play passthrough state.
    /// Used for idempotent enter/exit and safe cleanup.
    in_raw_passthrough: bool,
    /// Modes the host terminal currently has on for the Raw Play child. Default
    /// outside Raw Play.
    child_modes: ChildModes,
    /// Modes each session's app enabled, learned from that session's output
    /// whether or not Raw Play is showing it. Re-applied on Raw Play re-entry.
    session_modes: std::collections::HashMap<String, ChildModes>,
}

impl App {
    pub fn new(
        session_service: SessionService,
        terminal_service: TerminalService,
        runtime_rx: UnboundedReceiver<RuntimeEvent>,
    ) -> Self {
        let (planner_tx, planner_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            session_service,
            terminal_service,
            runtime_rx,
            planner_rx,
            planner_tx,
            ollama_config: Arc::new(OllamaConfig::default()),
            model: Model::new(),
            session_counter: 0,
            in_raw_passthrough: false,
            child_modes: ChildModes::default(),
            session_modes: std::collections::HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Armed before the terminal changes, so a failure between the two
        // setup calls still restores the host. Drop is idempotent.
        let _restore = ui::TerminalRestoreGuard::arm();
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        // No initial session — welcome banner shows first.
        // User presses ^N to create their first session.

        // Initial resize
        self.sync_pane_size(&terminal);

        let result = self.event_loop(&mut terminal).await;

        // If we were in raw passthrough, restore alternate screen first
        if self.in_raw_passthrough {
            let _ = stdout().execute(EnterAlternateScreen);
            reset_host_capture();
            self.child_modes = ChildModes::default();
            self.in_raw_passthrough = false;
        }

        // Cleanup — close all sessions
        let ids: Vec<String> = self.model.sessions.iter().map(|s| s.id.clone()).collect();
        for id in ids {
            let result = self.session_service.close(&id);
            self.model.surface_session_result(result);
        }

        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        result
    }

    /// Create a new session and add it to the model.
    fn create_session(&mut self) {
        self.session_counter += 1;
        let label = format!("Session {}", self.session_counter);

        match self.session_service.create(CreateSessionRequest {
            label: Some(label.clone()),
            cwd: None,
            shell: None,
        }) {
            Ok(summary) => {
                let idx = self.model.add_session(summary.id, label);
                self.model.sessions[idx].shell = Some(summary.shell);
                self.model.switch_to(idx);
                self.model.create_error = None;
            }
            Err(e) => {
                // The runtime registry has no such session, so the model gets no
                // row for it. The reason shows in the pane and the footer.
                let message = format!("Could not start {label}: {e}");
                self.model.status_line = Some(message.clone());
                self.model.create_error = Some(message);
            }
        }
    }

    /// Close the active session and switch to an adjacent one.
    /// If the closed session owns a pending proposal, clear it.
    /// If in raw play mode for this session, exit raw play first.
    /// The last session can close; the welcome state is a supported state.
    fn close_active_session(&mut self) {
        if self.model.session_count() == 0 {
            return;
        }
        let session_id = self.model.sessions[self.model.active_index].id.clone();
        self.close_session_by_id(&session_id);
    }

    /// Close one session by id. The session the user was on stays selected
    /// unless it is the one being closed.
    fn close_session_by_id(&mut self, session_id: &str) {
        if self.model.session_index(session_id).is_none() {
            return;
        }
        let session_id = session_id.to_string();
        self.session_modes.remove(&session_id);

        // If this session owns the current proposal, clear it
        if self.model.proposal_owner() == Some(session_id.as_str()) {
            self.model.clear_proposal();
            // If we were in Review mode for this session, go back to Shell
            if self.model.input_mode == InputMode::Review {
                self.model.input_mode = InputMode::Shell;
            }
        }

        // Close in runtime. A missing session stays visible as a one-line status.
        let result = self.session_service.close(&session_id);
        self.model.surface_session_result(result);

        // Remove from model. Closing a session that is not active leaves the
        // active one selected.
        self.model.remove_session_by_id(&session_id);
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        let mut resize_pending = false;

        loop {
            if self.model.input_mode == InputMode::RawPlay {
                // --- Raw play mode loop ---
                // PTY output goes to stdout, keys go to PTY, Console chrome hidden.
                self.raw_play_tick(terminal)?;
            } else {
                // --- Normal Console mode loop ---

                // 1. Drain runtime events — routed by session_id. A budget per
                // frame: a flood (`yes`) must not starve the render and key poll,
                // or Ctrl+C could never be read. The rest waits for the next frame.
                self.drain_events(EVENTS_PER_FRAME);

                // 2. Drain planner results — proposals are session-bound
                while let Ok(result) = self.planner_rx.try_recv() {
                    let epoch = match &result {
                        PlannerResult::Success(_, epoch) | PlannerResult::Error(_, epoch) => *epoch,
                    };
                    // A cancelled request must not touch the current state.
                    if epoch != self.model.planner_epoch {
                        continue;
                    }
                    self.model.planner_busy = false;
                    match result {
                        PlannerResult::Success((proposal, session_id), _) => {
                            if self.model.session_index(&session_id).is_some() {
                                self.model.set_proposal(proposal, session_id);
                                self.model.input_mode = InputMode::Review;
                                // Keys pressed while the planner ran must not
                                // approve what the user has not seen.
                                self.model.arm_review_debounce();
                                if drain_pending_input()? {
                                    resize_pending = true;
                                }
                                self.sync_pane_size(terminal);
                            }
                        }
                        PlannerResult::Error(msg, _) => {
                            self.model.planner_error = Some(msg);
                        }
                    }
                }

                // 3. Apply coalesced resize
                if resize_pending {
                    self.sync_pane_size(terminal);
                    resize_pending = false;
                }

                // 4. Render
                terminal.draw(|frame| {
                    ui::render(frame, &mut self.model);
                })?;

                // 5. Poll for crossterm events
                if ct_event::poll(Duration::from_millis(16))? {
                    match ct_event::read()? {
                        Event::Key(key) => {
                            if key.kind == KeyEventKind::Press {
                                let action = input::handle_key(
                                    key,
                                    &mut self.model,
                                    &self.terminal_service,
                                );
                                self.handle_action(action, terminal);
                            }
                        }
                        Event::Resize(_cols, _rows) => {
                            resize_pending = true;
                        }
                        _ => {}
                    }
                }
            }

            if self.model.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// One tick of the raw play mode loop.
    /// PTY output written directly to host stdout. Keys forwarded to PTY.
    /// Detects session death and auto-exits to Console.
    fn raw_play_tick(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        // Check if the active session is still alive — auto-exit if dead
        if !self.model.active_session_alive() {
            self.model.input_mode = InputMode::Shell;
            self.exit_raw_play(terminal);
            return Ok(());
        }

        let active_id = self.model.active_session_id().map(|s| s.to_string());

        // Drain runtime events — active session output goes directly to stdout.
        // Same per-frame budget as Console mode, so keys are still polled in a flood.
        for _ in 0..EVENTS_PER_FRAME {
            let Ok(event) = self.runtime_rx.try_recv() else {
                break;
            };
            if let RuntimeEvent::TerminalLine(ref line_event) = event {
                // Every session's mode changes are learned, not only the active
                // session's, so a background session's toggles are not missed.
                self.observe_session_modes(&event);
                if active_id.as_deref() == Some(line_event.session_id.as_str()) {
                    let mut out = stdout();
                    let _ = out.write_all(line_event.text.as_bytes());
                    let _ = out.flush();
                    // Apply what the child wants forwarded.
                    let learned = self
                        .session_modes
                        .get(&line_event.session_id)
                        .copied()
                        .unwrap_or_default();
                    apply_host_capture(self.child_modes, learned);
                    self.child_modes = learned;
                }
                // Non-active session output is NOT written to stdout (targeting truth)
            }
            // All events still route to the model for state tracking + unread
            self.model.apply_event(event);
        }

        // Poll for crossterm events
        if ct_event::poll(Duration::from_millis(8))? {
            match ct_event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        let action = input::handle_key(
                            key,
                            &mut self.model,
                            &self.terminal_service,
                        );
                        self.handle_action(action, terminal);
                    }
                }
                Event::Resize(cols, rows) => {
                    // Full host dimensions — no chrome subtraction.
                    if cols > 0 && rows > 0 {
                        self.model.pane_cols = cols;
                        self.model.pane_rows = rows;
                        if let Some(ref session_id) = active_id {
                            self.resize_session(session_id, cols, rows);
                        }
                    }
                }
                Event::Paste(text) => {
                    // Crossterm stripped the paste markers. Put them back when the
                    // child enabled bracketed paste.
                    if !text.is_empty() {
                        if let Some(ref session_id) = active_id {
                            let running = self.model.session_is_running(session_id);
                            if paste_refused(running, text.len()) {
                                // The PTY write blocks the only UI thread when the
                                // child is not reading. Refuse, with a bell, and say
                                // why once Console is back.
                                let mut out = stdout();
                                let _ = out.write_all(b"\x07");
                                let _ = out.flush();
                                self.model.status_line = Some(format!(
                                    "Paste of {} bytes refused while a command is running (limit {} bytes)",
                                    text.len(),
                                    RUNNING_PASTE_LIMIT
                                ));
                            } else {
                                let data = self.child_modes.encode_paste(&text);
                                let result = self.terminal_service.write(session_id, &data);
                                self.raw_write_result(session_id, result, terminal);
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let (Some(session_id), Some(data)) =
                        (active_id.as_ref(), self.child_modes.encode_mouse(&mouse))
                    {
                        let result = self.terminal_service.write(session_id, &data);
                        self.raw_write_result(session_id, result, terminal);
                    }
                }
                focus @ (Event::FocusGained | Event::FocusLost) => {
                    let gained = focus == Event::FocusGained;
                    if let (Some(session_id), Some(data)) =
                        (active_id.as_ref(), self.child_modes.encode_focus(gained))
                    {
                        let result = self.terminal_service.write(session_id, &data);
                        self.raw_write_result(session_id, result, terminal);
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply at most `budget` queued runtime events. Returns how many were applied.
    fn drain_events(&mut self, budget: usize) -> usize {
        let mut applied = 0;
        while applied < budget {
            let Ok(event) = self.runtime_rx.try_recv() else {
                break;
            };
            self.observe_session_modes(&event);
            self.model.apply_event(event);
            applied += 1;
        }
        applied
    }

    /// Outcome of a Raw Play write that is not a key (paste, mouse, focus).
    /// A failed write takes the same path as a failed key write: the session is
    /// marked as an error and Console comes back, since Raw Play draws no status line.
    fn raw_write_result(
        &mut self,
        session_id: &str,
        result: Result<(), String>,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        if let Err(err) = &result {
            if let Some(idx) = self.model.session_index(session_id) {
                self.model.sessions[idx].session_state = crate::model::SessionState::Error(err.clone());
            }
            self.model.surface_session_result(result);
            self.model.input_mode = InputMode::Shell;
            self.exit_raw_play(terminal);
        } else {
            self.model.surface_session_result(result);
        }
    }

    fn handle_action(
        &mut self,
        action: InputAction,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        match action {
            InputAction::Quit => {
                self.model.should_quit = true;
            }
            InputAction::SubmitIntent(intent) => {
                self.spawn_planner(intent);
            }
            InputAction::ApproveProposal(command, target_session_id, plan_id) => {
                self.execute_on_session(&command, &target_session_id, &plan_id);
                self.sync_pane_size(terminal);
            }
            InputAction::CancelProposal => {
                self.sync_pane_size(terminal);
            }
            InputAction::CreateSession => {
                self.create_session();
                self.sync_pane_size(terminal);
            }
            InputAction::CloseSession => {
                self.close_active_session();
                self.sync_pane_size(terminal);
            }
            InputAction::CloseSessionAt(id) => {
                self.close_session_by_id(&id);
                self.sync_pane_size(terminal);
            }
            InputAction::NextSession | InputAction::PrevSession => {
                self.sync_pane_size(terminal);
            }
            InputAction::EnterRawPlay => {
                self.enter_raw_play(terminal);
            }
            InputAction::ExitRawPlay => {
                self.exit_raw_play(terminal);
            }
            InputAction::ModeSwitched => {
                self.sync_pane_size(terminal);
            }
            _ => {}
        }
    }

    /// Enter raw play mode — surrender terminal to the game.
    /// Idempotent: safe to call if already in passthrough.
    fn enter_raw_play(&mut self, _terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
        if self.in_raw_passthrough {
            return; // Already in raw mode
        }

        // Leave Ratatui's alternate screen — gives host terminal back
        let _ = stdout().execute(LeaveAlternateScreen);

        // Clear screen so the game starts fresh
        let _ = crossterm::execute!(
            stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0),
            crossterm::cursor::Show
        );

        // Show entry hint
        let _ = write!(
            stdout(),
            "\x1b[2m── Raw Play: game has the terminal. Press Ctrl+\\ to return. ──\x1b[0m\r\n\r\n"
        );
        let _ = stdout().flush();

        // Record the fullscreen size we send. Exit recomputes the chrome pane
        // and ioctls that, so the restore value is the pane, not this size.
        if let Ok((cols, rows)) = crossterm::terminal::size() {
            if cols > 0 && rows > 0 {
                self.model.pane_cols = cols;
                self.model.pane_rows = rows;
                if let Some(session_id) = self.model.active_session_id().map(|s| s.to_string()) {
                    self.resize_session(&session_id, cols, rows);
                }
            }
        }

        // The app may have enabled mouse, focus or paste modes earlier (before a
        // peek at Console). Turn the host's capture back on to match.
        let learned = self
            .model
            .active_session_id()
            .and_then(|id| self.session_modes.get(id).copied())
            .unwrap_or_default();
        apply_host_capture(self.child_modes, learned);
        self.child_modes = learned;

        self.in_raw_passthrough = true;
    }

    /// Track the modes a session's app enabled from its own output.
    fn observe_session_modes(&mut self, event: &RuntimeEvent) {
        if let RuntimeEvent::TerminalLine(line) = event {
            self.session_modes
                .entry(line.session_id.clone())
                .or_default()
                .observe(&line.text);
        }
    }

    /// Exit raw play mode — Console resumes.
    /// Idempotent: safe to call if not in passthrough.
    fn exit_raw_play(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
        if !self.in_raw_passthrough {
            return; // Not in raw mode
        }

        // Re-enter alternate screen for Ratatui
        let _ = stdout().execute(EnterAlternateScreen);

        // Hide cursor (Ratatui manages it)
        let _ = crossterm::execute!(stdout(), crossterm::cursor::Hide);

        // Force full redraw
        let _ = terminal.clear();

        // Callers set Shell before exit. If they did not, do it here so the
        // restore size is the chrome pane, not the fullscreen raw-play size.
        if self.model.input_mode == InputMode::RawPlay {
            self.model.input_mode = InputMode::Shell;
        }

        // Always ioctl the pane the user is looking at, even when the stored
        // size already matches.
        self.sync_pane_size(terminal);

        // Give the host its own mouse, focus and paste handling back.
        // The host gets its own handling back. The app keeps its modes: they stay
        // in session_modes and are re-applied on re-entry.
        // Unconditional: the host turned these on from the child's raw bytes even
        // when Console would not have forwarded them (?1000 without ?1006).
        reset_host_capture();
        self.child_modes = ChildModes::default();
        self.in_raw_passthrough = false;
    }

    fn spawn_planner(&self, intent: String) {
        let config = self.ollama_config.clone();
        let tx = self.planner_tx.clone();

        // Capture the session ID at Ask time — this is the proposal owner
        let session_id = self.model.active_session_id().unwrap_or("").to_string();
        let epoch = self.model.planner_epoch;

        let context = planner::build_context_for_shell(
            &session_id,
            self.model
                .active_session()
                .and_then(|s| s.cwd.as_deref())
                .unwrap_or("."),
            self.model
                .active_session()
                .and_then(|s| s.shell.as_deref()),
        );

        tokio::spawn(async move {
            let proposal = planner::generate_proposal(&config, &context, &intent).await;
            let _ = tx.send(PlannerResult::Success((proposal, session_id), epoch));
        });
    }

    /// Execute a command on a specific session — not necessarily the active one.
    /// This is the proposal targeting truth: approval executes on the proposal's session.
    /// The proposal stays in Review until execute returns Ok.
    fn execute_on_session(&mut self, command: &str, session_id: &str, plan_id: &str) {
        let request = ask_execute_request(
            uuid::Uuid::new_v4().to_string(),
            session_id,
            command,
            plan_id,
        );
        let result = self.terminal_service.execute(request).map(|_| ());
        apply_execute_result(&mut self.model, result);
    }

    fn resize_session(&mut self, session_id: &str, cols: u16, rows: u16) {
        let result = self.terminal_service.resize(session_id, cols, rows);
        self.model.surface_session_result(result);
    }

    fn sync_pane_size(&mut self, terminal: &Terminal<CrosstermBackend<std::io::Stdout>>) {
        let area = terminal.size().unwrap_or_default();
        let Some((cols, rows)) = pane_size_for(self.model.input_mode.clone(), area.width, area.height)
        else {
            return;
        };

        self.model.pane_cols = cols;
        self.model.pane_rows = rows;

        // Resize the ACTIVE session's PTY only
        if let Some(session_id) = self.model.active_session_id().map(|s| s.to_string()) {
            self.resize_session(&session_id, cols, rows);
        }
    }
}

/// PTY size for the chrome pane in `mode` on a `width` x `height` host.
/// `None` when nothing fits. Raw Play has no chrome, so the whole host is the pane.
fn pane_size_for(mode: InputMode, width: u16, height: u16) -> Option<(u16, u16)> {
    let chrome_overhead = match mode {
        InputMode::Shell | InputMode::Switcher => 4,
        InputMode::Ask => 6,
        InputMode::Review => 14,
        InputMode::RawPlay => 0,
    };
    let cols = width.saturating_sub(2);
    let rows = height.saturating_sub(chrome_overhead);
    if cols == 0 || rows == 0 {
        None
    } else {
        Some((cols, rows))
    }
}

/// Modes the child enabled by writing DEC private mode sequences to its output.
/// Raw Play forwards host events only in the forms the child asked for.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct ChildModes {
    /// `?2004`: wrap pastes in `ESC[200~` .. `ESC[201~`.
    bracketed_paste: bool,
    /// `?1000`: report button press and release.
    mouse_press: bool,
    /// `?1002`: also report motion while a button is held.
    mouse_drag: bool,
    /// `?1003`: report all motion, with or without a button.
    mouse_motion: bool,
    /// `?1006`: SGR mouse encoding, the only one forwarded (the PTY write is text).
    mouse_sgr: bool,
    /// `?1004`: report focus in and out.
    focus: bool,
}

/// Runtime events applied per frame before Console renders and polls keys.
const EVENTS_PER_FRAME: usize = 256;

/// The largest paste written to a session with a running command. A child that
/// is not reading stdin fills the PTY input queue (a few KB) and the write blocks.
const RUNNING_PASTE_LIMIT: usize = 4096;

/// Whether a paste of `len` bytes must be refused.
fn paste_refused(running: bool, len: usize) -> bool {
    running && len > RUNNING_PASTE_LIMIT
}

/// Turn mouse, focus and bracketed-paste reporting off on the host terminal,
/// whatever Console believes is on. Safe to repeat.
fn reset_host_capture() {
    use crossterm::event::{DisableBracketedPaste, DisableFocusChange, DisableMouseCapture};
    let mut out = stdout();
    let _ = out.execute(DisableMouseCapture);
    let _ = out.execute(DisableFocusChange);
    let _ = out.execute(DisableBracketedPaste);
}

/// Turn the host terminal's mouse, focus and bracketed-paste reporting on or
/// off to match what the child enabled. Only changes are written.
fn apply_host_capture(before: ChildModes, after: ChildModes) {
    use crossterm::event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture,
    };
    let mut out = stdout();
    // Host capture only when the app's mouse reports can actually be forwarded;
    // otherwise the host would swallow the mouse for nothing.
    if before.mouse_forwardable() != after.mouse_forwardable() {
        let _ = if after.mouse_forwardable() {
            out.execute(EnableMouseCapture).map(|_| ())
        } else {
            out.execute(DisableMouseCapture).map(|_| ())
        };
    }
    if before.focus != after.focus {
        let _ = if after.focus {
            out.execute(EnableFocusChange).map(|_| ())
        } else {
            out.execute(DisableFocusChange).map(|_| ())
        };
    }
    if before.bracketed_paste != after.bracketed_paste {
        let _ = if after.bracketed_paste {
            out.execute(EnableBracketedPaste).map(|_| ())
        } else {
            out.execute(DisableBracketedPaste).map(|_| ())
        };
    }
}

impl ChildModes {
    /// Any mouse reporting level is on.
    fn mouse_on(&self) -> bool {
        self.mouse_press || self.mouse_drag || self.mouse_motion
    }

    /// Mouse reports can reach the app: it asked for them and for SGR encoding.
    fn mouse_forwardable(&self) -> bool {
        self.mouse_on() && self.mouse_sgr
    }

    /// Scan child output for `ESC [ ? <n>;<n> h|l`. A sequence split across two
    /// chunks is missed; the next toggle corrects it.
    fn observe(&mut self, text: &str) {
        let mut rest = text;
        while let Some(pos) = rest.find("\u{1b}[?") {
            let after = &rest[pos + 3..];
            let Some(end) = after.find(|c: char| !(c.is_ascii_digit() || c == ';')) else {
                break;
            };
            let on = match after[end..].chars().next() {
                Some('h') => Some(true),
                Some('l') => Some(false),
                _ => None,
            };
            if let Some(on) = on {
                for param in after[..end].split(';') {
                    match param {
                        "2004" => self.bracketed_paste = on,
                        "1000" => self.mouse_press = on,
                        "1002" => self.mouse_drag = on,
                        "1003" => self.mouse_motion = on,
                        "1006" => self.mouse_sgr = on,
                        "1004" => self.focus = on,
                        _ => {}
                    }
                }
            }
            rest = &after[end..];
        }
    }

    /// Paste text as the child expects it: bracketed only when it asked.
    fn encode_paste(&self, text: &str) -> String {
        if self.bracketed_paste {
            format!("\u{1b}[200~{text}\u{1b}[201~")
        } else {
            text.to_string()
        }
    }

    fn encode_focus(&self, gained: bool) -> Option<String> {
        self.focus
            .then(|| if gained { "\u{1b}[I" } else { "\u{1b}[O" }.to_string())
    }

    /// SGR mouse report, or `None` when the child did not enable it.
    fn encode_mouse(&self, event: &ct_event::MouseEvent) -> Option<String> {
        use ct_event::{KeyModifiers, MouseButton, MouseEventKind};
        if !self.mouse_forwardable() {
            return None;
        }
        let button = |b: &MouseButton| match b {
            MouseButton::Left => 0u16,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        };
        let (mut code, release) = match &event.kind {
            MouseEventKind::Down(b) => (button(b), false),
            MouseEventKind::Up(b) => (button(b), true),
            MouseEventKind::Drag(b) => {
                // Drag reports need ?1002 or ?1003; ?1000 asked for press/release only.
                if !(self.mouse_drag || self.mouse_motion) {
                    return None;
                }
                (button(b) + 32, false)
            }
            MouseEventKind::Moved => {
                if !self.mouse_motion {
                    return None;
                }
                (35, false)
            }
            MouseEventKind::ScrollUp => (64, false),
            MouseEventKind::ScrollDown => (65, false),
            MouseEventKind::ScrollLeft => (66, false),
            MouseEventKind::ScrollRight => (67, false),
        };
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            code += 4;
        }
        if event.modifiers.contains(KeyModifiers::ALT) {
            code += 8;
        }
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            code += 16;
        }
        let last = if release { 'm' } else { 'M' };
        Some(format!(
            "\u{1b}[<{code};{};{}{last}",
            event.column.saturating_add(1),
            event.row.saturating_add(1)
        ))
    }
}

/// Discard key presses that queued up while the planner ran. Returns true when
/// a resize was seen, so the caller can still apply it.
fn drain_pending_input() -> anyhow::Result<bool> {
    let mut resized = false;
    while ct_event::poll(Duration::ZERO)? {
        if let Event::Resize(..) = ct_event::read()? {
            resized = true;
        }
    }
    Ok(resized)
}

fn ask_execute_request(
    execution_id: String,
    session_id: &str,
    command: &str,
    plan_id: &str,
) -> ExecuteRequest {
    ExecuteRequest {
        execution_id,
        session_id: session_id.to_string(),
        command: command.to_string(),
        source: "ask".to_string(),
        linked_plan_id: Some(plan_id.to_string()),
    }
}

/// Ok leaves Review and drops the proposal. Err stays on the command and shows the error.
fn apply_execute_result(model: &mut Model, result: Result<(), String>) {
    match result {
        Ok(()) => {
            model.input_mode = InputMode::Shell;
            model.composer_clear();
            model.clear_proposal();
        }
        Err(err) => {
            model.input_mode = InputMode::Review;
            model.review_error = Some(err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CommandProposal;
    use commandui_runtime_core::events::{NoopSink, RuntimeEventSink};
    use commandui_runtime_core::session::SessionRegistry;
    use std::sync::{Arc, Mutex};

    fn test_app() -> App {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(NoopSink);
        let session_service = SessionService::new(sessions.clone(), sink.clone());
        let terminal_service = TerminalService::new(sessions, sink);
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        App::new(session_service, terminal_service, rx)
    }

    fn proposal(command: &str) -> CommandProposal {
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
    fn a_flood_of_output_is_drained_in_bounded_slices() {
        use commandui_runtime_core::events::TerminalLineEvent;
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(NoopSink);
        let session_service = SessionService::new(sessions.clone(), sink.clone());
        let terminal_service = TerminalService::new(sessions, sink);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(session_service, terminal_service, rx);
        app.model.add_session("s1".into(), "A".into());
        for i in 0..(EVENTS_PER_FRAME * 3) {
            tx.send(RuntimeEvent::TerminalLine(TerminalLineEvent {
                id: format!("l{i}"),
                session_id: "s1".into(),
                execution_id: None,
                kind: "stdout".into(),
                text: "y\n".into(),
                timestamp: "t".into(),
            }))
            .unwrap();
        }
        // One frame applies the budget and no more; the rest waits for the next.
        assert_eq!(app.drain_events(EVENTS_PER_FRAME), EVENTS_PER_FRAME);
        assert_eq!(app.model.sessions[0].terminal_lines.len(), EVENTS_PER_FRAME);
        assert_eq!(app.drain_events(EVENTS_PER_FRAME), EVENTS_PER_FRAME);
        assert_eq!(app.drain_events(EVENTS_PER_FRAME), EVENTS_PER_FRAME);
        assert_eq!(app.drain_events(EVENTS_PER_FRAME), 0);
    }

    #[test]
    fn a_big_paste_is_refused_only_while_a_command_runs() {
        assert!(paste_refused(true, RUNNING_PASTE_LIMIT + 1));
        assert!(!paste_refused(true, RUNNING_PASTE_LIMIT));
        assert!(!paste_refused(false, RUNNING_PASTE_LIMIT * 100));
    }

    #[test]
    fn a_session_shell_reaches_the_planner_context() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model.sessions[0].shell = Some("pwsh.exe".into());
        let shell = app.model.active_session().and_then(|s| s.shell.as_deref());
        let ctx = planner::build_context_for_shell("s1", ".", shell);
        assert_eq!(ctx.shell, "pwsh.exe");
    }

    #[test]
    fn execute_error_keeps_the_proposal_and_shows_the_error() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model
            .set_proposal(proposal("echo hi"), "s1".into());
        app.model.input_mode = InputMode::Review;
        app.model.composer_text = "keep".into();

        // Single line: runtime-core now rejects a multi-line command before
        // the session lookup this test exercises.
        app.execute_on_session("echo hi", "s1", "plan-9");

        assert_eq!(app.model.input_mode, InputMode::Review);
        assert_eq!(
            app.model.current_proposal.as_ref().unwrap().command,
            "echo hi"
        );
        assert_eq!(app.model.proposal_session_id.as_deref(), Some("s1"));
        assert_eq!(app.model.composer_text, "keep");
        let err = app.model.review_error.as_deref().unwrap();
        assert!(err.contains("Session not found"), "{err}");
    }

    #[test]
    fn execute_ok_clears_the_proposal() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model.set_proposal(proposal("echo hi"), "s1".into());
        app.model.input_mode = InputMode::Review;
        app.model.composer_text = "typed".into();
        app.model.review_error = Some("stale".into());

        apply_execute_result(&mut app.model, Ok(()));

        assert_eq!(app.model.input_mode, InputMode::Shell);
        assert!(app.model.current_proposal.is_none());
        assert!(app.model.composer_text.is_empty());
        assert!(app.model.review_error.is_none());
    }

    #[test]
    fn execute_request_links_the_proposal_id() {
        let request = ask_execute_request("e1".into(), "s1", "echo hi\nwhoami", "plan-9");
        assert_eq!(request.linked_plan_id.as_deref(), Some("plan-9"));
        assert_eq!(request.command, "echo hi\nwhoami");
        assert_eq!(request.session_id, "s1");
        assert_eq!(request.source, "ask");
    }

    #[test]
    fn resize_and_close_errors_are_not_swallowed() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model.add_session("s2".into(), "B".into());

        app.resize_session("s1", 80, 24);
        let status = app.model.status_line.clone().unwrap();
        assert!(status.contains("Session not found"), "{status}");

        app.model.status_line = None;
        app.close_active_session();
        let status = app.model.status_line.as_deref().unwrap();
        assert!(status.contains("Session not found"), "{status}");
    }

    #[test]
    fn closing_the_proposal_owner_clears_review_and_the_last_session_can_close() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model.add_session("s2".into(), "B".into());
        app.model.switch_to(1);
        app.model.set_proposal(proposal("echo hi"), "s2".into());
        app.model.input_mode = InputMode::Review;
        app.close_active_session();
        assert!(app.model.current_proposal.is_none());
        assert_eq!(app.model.input_mode, InputMode::Shell);
        assert_eq!(app.model.session_count(), 1);
        assert_eq!(app.model.active_index, 0);

        // The last session closes too, back to the empty welcome state.
        app.close_active_session();
        assert_eq!(app.model.session_count(), 0);
        assert_eq!(app.model.active_index, 0);
        app.close_active_session();
        assert_eq!(app.model.session_count(), 0);
    }

    #[test]
    fn closing_the_active_session_clears_unread_on_the_one_it_lands_on() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model.add_session("s2".into(), "B".into());
        app.model.add_session("s3".into(), "C".into());
        app.model.switch_to(1);
        app.model.sessions[2].has_unread = true;
        app.close_active_session();
        // s3 slid into index 1 and is now the session on screen.
        assert_eq!(app.model.active_session_id(), Some("s3"));
        assert!(!app.model.sessions[1].has_unread);
    }

    #[test]
    fn pane_size_is_the_chrome_pane_and_raw_play_is_the_whole_host() {
        assert_eq!(pane_size_for(InputMode::Shell, 100, 40), Some((98, 36)));
        assert_eq!(pane_size_for(InputMode::Ask, 100, 40), Some((98, 34)));
        assert_eq!(pane_size_for(InputMode::Review, 100, 40), Some((98, 26)));
        assert_eq!(pane_size_for(InputMode::RawPlay, 100, 40), Some((98, 40)));
        assert_eq!(pane_size_for(InputMode::Shell, 2, 40), None);
        assert_eq!(pane_size_for(InputMode::Review, 100, 14), None);
    }

    #[test]
    fn child_modes_follow_the_sequences_the_child_wrote() {
        let mut modes = ChildModes::default();
        modes.observe("hello \u{1b}[?1000;1006h and \u{1b}[?2004h");
        assert!(modes.mouse_on() && modes.mouse_sgr && modes.bracketed_paste);
        assert!(!modes.focus && !modes.mouse_motion);
        modes.observe("\u{1b}[?1004h\u{1b}[?1003h");
        assert!(modes.focus && modes.mouse_motion);
        modes.observe("\u{1b}[?1000l\u{1b}[?1003l\u{1b}[?2004l");
        assert!(!modes.mouse_on() && !modes.bracketed_paste);
        // ?1003l does not clear ?1000, and ?1000 alone does not forward drags.
        let mut modes = ChildModes::default();
        modes.observe("\u{1b}[?1000;1006h\u{1b}[?1003h\u{1b}[?1003l");
        assert!(modes.mouse_on() && !modes.mouse_motion);
        let drag = ct_event::MouseEvent {
            kind: ct_event::MouseEventKind::Drag(ct_event::MouseButton::Left),
            column: 1,
            row: 1,
            modifiers: ct_event::KeyModifiers::NONE,
        };
        assert!(modes.encode_mouse(&drag).is_none());
        modes.observe("\u{1b}[?1002h");
        assert!(modes.encode_mouse(&drag).is_some());
        // Without SGR nothing is forwardable, so the host must not capture.
        let mut modes = ChildModes::default();
        modes.observe("\u{1b}[?1000h");
        assert!(!modes.mouse_forwardable());
    }

    #[test]
    fn raw_play_forwards_paste_mouse_and_focus_in_the_child_encoding() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let mut modes = ChildModes::default();
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: 9,
            modifiers: KeyModifiers::NONE,
        };
        // Nothing enabled: paste is plain text, mouse and focus are not sent.
        assert_eq!(modes.encode_paste("ls"), "ls");
        assert_eq!(modes.encode_mouse(&click), None);
        assert_eq!(modes.encode_focus(true), None);

        modes.observe("\u{1b}[?2004h\u{1b}[?1000h\u{1b}[?1006h\u{1b}[?1004h");
        assert_eq!(modes.encode_paste("ls"), "\u{1b}[200~ls\u{1b}[201~");
        assert_eq!(modes.encode_mouse(&click).as_deref(), Some("\u{1b}[<0;5;10M"));
        let release = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Right),
            modifiers: KeyModifiers::CONTROL,
            ..click
        };
        assert_eq!(modes.encode_mouse(&release).as_deref(), Some("\u{1b}[<18;5;10m"));
        let moved = MouseEvent {
            kind: MouseEventKind::Moved,
            ..click
        };
        assert_eq!(modes.encode_mouse(&moved), None);
        assert_eq!(modes.encode_focus(true).as_deref(), Some("\u{1b}[I"));
        assert_eq!(modes.encode_focus(false).as_deref(), Some("\u{1b}[O"));
    }

    #[test]
    fn a_cancelled_planner_result_is_dropped_by_epoch() {
        let mut app = test_app();
        app.model.add_session("s1".into(), "A".into());
        app.model.planner_busy = true;
        let started = app.model.planner_epoch;
        app.model.cancel_planner();
        assert!(!app.model.planner_busy);
        assert_ne!(app.model.planner_epoch, started);
    }

    #[test]
    fn create_session_failure_adds_no_model_row_and_says_why() {
        let mut app = test_app();
        app.create_session();
        let id = app.model.sessions.first().map(|s| s.id.clone());
        match id {
            Some(id) => app.session_service.close(&id).expect("close spawned session"),
            None => {
                assert_eq!(app.model.session_count(), 0);
                let why = app.model.create_error.as_deref().unwrap();
                assert!(why.contains("Session 1"), "{why}");
                assert_eq!(app.model.status_line.as_deref(), Some(why));
            }
        }
        assert!(app.model.sessions.iter().all(|s| !s.id.starts_with("error-")));
    }

    #[test]
    fn spawn_planner_returns_a_mock_bound_to_the_asking_session() {

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut app = test_app();
            app.ollama_config = Arc::new(OllamaConfig {
                endpoint: "http://127.0.0.1:1".into(),
                model: "none".into(),
                timeout_secs: 1,
            });
            app.model.add_session("s1".into(), "A".into());
            app.model.sessions[0].cwd = Some("/work".into());
            app.spawn_planner("list the files".into());
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                app.planner_rx.recv(),
            )
            .await
            .expect("planner timed out")
            .expect("planner channel closed");
            match result {
                PlannerResult::Success((proposal, session_id), _) => {
                    assert_eq!(session_id, "s1");
                    assert!(!proposal.command.is_empty());
                }
                PlannerResult::Error(err, _) => panic!("planner error: {err}"),
            }
        });
    }
}
