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
use crate::model::{InputMode, Model, SessionState};
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
    Success((commandui_runtime_planner::CommandProposal, String)),
    Error(String),
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
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        // Create initial session
        self.create_session();

        // Initial resize
        self.sync_pane_size(&terminal);

        let result = self.event_loop(&mut terminal).await;

        // If we were in raw passthrough, restore alternate screen first
        if self.in_raw_passthrough {
            let _ = stdout().execute(EnterAlternateScreen);
            self.in_raw_passthrough = false;
        }

        // Cleanup — close all sessions
        for session in &self.model.sessions {
            let _ = self.session_service.close(&session.id);
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
                self.model.switch_to(idx);
            }
            Err(e) => {
                // Add a session in error state so the user sees something
                let idx = self.model.add_session(
                    format!("error-{}", self.session_counter),
                    label,
                );
                self.model.sessions[idx].session_state =
                    SessionState::Error(format!("Failed: {e}"));
                self.model.switch_to(idx);
            }
        }
    }

    /// Close the active session and switch to an adjacent one.
    /// If the closed session owns a pending proposal, clear it.
    /// If in raw play mode for this session, exit raw play first.
    fn close_active_session(&mut self) {
        if self.model.session_count() <= 1 {
            return; // Never close the last session
        }

        let idx = self.model.active_index;
        let session_id = self.model.sessions[idx].id.clone();

        // If this session owns the current proposal, clear it
        if self.model.proposal_owner() == Some(session_id.as_str()) {
            self.model.clear_proposal();
            // If we were in Review mode for this session, go back to Shell
            if self.model.input_mode == InputMode::Review {
                self.model.input_mode = InputMode::Shell;
            }
        }

        // Close in runtime
        let _ = self.session_service.close(&session_id);

        // Remove from model
        self.model.sessions.remove(idx);

        // Adjust active index
        if self.model.active_index >= self.model.sessions.len() {
            self.model.active_index = self.model.sessions.len().saturating_sub(1);
        }
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

                // 1. Drain runtime events — routed by session_id
                while let Ok(event) = self.runtime_rx.try_recv() {
                    self.model.apply_event(event);
                }

                // 2. Drain planner results — proposals are session-bound
                while let Ok(result) = self.planner_rx.try_recv() {
                    self.model.planner_busy = false;
                    match result {
                        PlannerResult::Success((proposal, session_id)) => {
                            if self.model.session_index(&session_id).is_some() {
                                self.model.set_proposal(proposal, session_id);
                                self.model.input_mode = InputMode::Review;
                            }
                        }
                        PlannerResult::Error(msg) => {
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
                    ui::render(frame, &self.model);
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

        // Drain runtime events — active session output goes directly to stdout
        while let Ok(event) = self.runtime_rx.try_recv() {
            if let RuntimeEvent::TerminalLine(ref line_event) = event {
                if active_id.as_deref() == Some(line_event.session_id.as_str()) {
                    let mut out = stdout();
                    let _ = out.write_all(line_event.text.as_bytes());
                    let _ = out.flush();
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
                    // Full host dimensions — no chrome subtraction
                    if let Some(ref session_id) = active_id {
                        let _ = self.terminal_service.resize(session_id, cols, rows);
                    }
                }
                _ => {}
            }
        }

        Ok(())
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
            InputAction::ApproveProposal(command, target_session_id) => {
                self.execute_on_session(&command, &target_session_id);
            }
            InputAction::CreateSession => {
                self.create_session();
            }
            InputAction::CloseSession => {
                self.close_active_session();
            }
            InputAction::NextSession | InputAction::PrevSession => {
                if let Some(ref id) = self.model.active_session_id().map(|s| s.to_string()) {
                    let _ = self.terminal_service.resize(
                        id,
                        self.model.pane_cols,
                        self.model.pane_rows,
                    );
                }
            }
            InputAction::EnterRawPlay => {
                self.enter_raw_play(terminal);
            }
            InputAction::ExitRawPlay => {
                self.exit_raw_play(terminal);
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

        // Resize PTY to full host terminal dimensions (no chrome overhead)
        if let Ok(size) = crossterm::terminal::size() {
            if let Some(session_id) = self.model.active_session_id().map(|s| s.to_string()) {
                let _ = self.terminal_service.resize(&session_id, size.0, size.1);
            }
        }

        self.in_raw_passthrough = true;
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

        // Resize PTY back to Console's pane dimensions
        self.sync_pane_size(terminal);

        self.in_raw_passthrough = false;
    }

    fn spawn_planner(&self, intent: String) {
        let config = self.ollama_config.clone();
        let tx = self.planner_tx.clone();

        // Capture the session ID at Ask time — this is the proposal owner
        let session_id = self.model.active_session_id().unwrap_or("").to_string();

        let context = planner::build_context(
            &session_id,
            &self.model.active_session().and_then(|s| s.cwd.as_deref()).unwrap_or("."),
        );

        tokio::spawn(async move {
            let proposal = planner::generate_proposal(&config, &context, &intent).await;
            let _ = tx.send(PlannerResult::Success((proposal, session_id)));
        });
    }

    /// Execute a command on a specific session — not necessarily the active one.
    /// This is the proposal targeting truth: approval executes on the proposal's session.
    fn execute_on_session(&self, command: &str, session_id: &str) {
        let request = ExecuteRequest {
            execution_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            command: command.to_string(),
            source: "ask".to_string(),
            linked_plan_id: None,
        };
        let _ = self.terminal_service.execute(request);
    }

    fn sync_pane_size(&mut self, terminal: &Terminal<CrosstermBackend<std::io::Stdout>>) {
        let area = terminal.size().unwrap_or_default();

        let chrome_overhead = match self.model.input_mode {
            InputMode::Shell | InputMode::Switcher => 4,
            InputMode::Ask => 6,
            InputMode::Review => 14,
            InputMode::RawPlay => 0, // No chrome in raw mode
        };

        let cols = area.width.saturating_sub(2);
        let rows = area.height.saturating_sub(chrome_overhead);

        if cols == 0 || rows == 0 {
            return;
        }

        if cols == self.model.pane_cols && rows == self.model.pane_rows {
            return;
        }

        self.model.pane_cols = cols;
        self.model.pane_rows = rows;

        // Resize the ACTIVE session's PTY only
        if let Some(session_id) = self.model.active_session_id().map(|s| s.to_string()) {
            let _ = self.terminal_service.resize(&session_id, cols, rows);
        }
    }
}
