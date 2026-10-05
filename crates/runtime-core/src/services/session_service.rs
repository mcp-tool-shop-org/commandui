//! Session lifecycle service.
//!
//! Owns session creation, listing, closing, and the reader-loop
//! state machine (prompt detection, boot readiness, cwd tracking,
//! execution completion inference).
//!
//! No adapter-specific types (Tauri, Ratatui) belong here.

use crate::events::{
    ExecutionFinishedEvent, RuntimeEvent, RuntimeEventSink, SessionCwdChangedEvent,
    SessionExecStateChangedEvent, SessionReadyEvent, TerminalLineEvent,
};
use crate::pty::{
    bootstrap_prompt, clone_reader, cmd_probe_line, default_shell, new_marker_nonce, shell_family,
    spawn_reader_with_exit, spawn_shell_with_args, strip_cmd_echo, write_raw, launch_args,
    resolve_default_session_cwd, CmdTail, Marker, MarkerKind,
    PtyHandle, ReaderEvent, ShellFamily,
};
use crate::session::{SessionExecState, SessionRecord, SessionRegistry, SessionTracking};
use std::sync::{Arc, Mutex};

/// The exit code reported for a command that finished without ever reporting
/// one: a cmd command whose tail never ran (a line that did not parse, a
/// command that ended the chain) and whose follow-up probe did not answer.
pub(crate) const UNKNOWN_EXIT_CODE: i32 = 1;
/// The exit code reported for a command that was interrupted and whose shell
/// does not know its code (cmd's Ctrl+C drops the rest of the line).
const INTERRUPTED_EXIT_CODE: i32 = 130;

/// Summary of a session — returned by create/list operations.
/// No Tauri types. Adapters map this to their own response shapes.
#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub id: String,
    pub label: String,
    pub cwd: String,
    pub shell: String,
    pub status: String,
    pub created_at: String,
    pub last_active_at: String,
}

/// Request to create a new session.
pub struct CreateSessionRequest {
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub shell: Option<String>,
}

pub struct SessionService {
    sessions: Arc<Mutex<SessionRegistry>>,
    event_sink: Arc<dyn RuntimeEventSink>,
}

impl SessionService {
    pub fn new(
        sessions: Arc<Mutex<SessionRegistry>>,
        event_sink: Arc<dyn RuntimeEventSink>,
    ) -> Self {
        Self {
            sessions,
            event_sink,
        }
    }

    pub fn create(&self, request: CreateSessionRequest) -> Result<SessionSummary, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let label = request.label.unwrap_or_else(|| "Session".to_string());
        let shell = request.shell.unwrap_or_else(default_shell);
        // Resolve the default cwd before anything is spawned: the process
        // folder, or the user's home folder when that is the Windows folder
        // (a packaged app starts in System32) or cannot be read.
        let cwd = match request.cwd {
            Some(cwd) => cwd,
            None => resolve_default_session_cwd()
                .ok_or_else(|| "Failed to resolve working directory".to_string())?
                .to_string_lossy()
                .to_string(),
        };
        let now = chrono::Utc::now().to_rfc3339();
        let nonce = new_marker_nonce();
        let Some(prompt_cmd) = bootstrap_prompt(&shell, &nonce) else {
            eprintln!("[session] refused unsupported shell: {shell}");
            return Err(format!("unsupported shell: {shell}"));
        };

        // PowerShell is set up by its launch arguments; the others are typed
        // the bootstrap once the reader is running.
        let args = launch_args(&shell, &nonce);
        let typed_bootstrap = args.is_empty();
        let (pair, writer, child) = spawn_shell_with_args(&shell, Some(&cwd), &args)?;
        let reader = clone_reader(&pair)?;
        let bootstrap_writer = writer.clone();

        let mut track = SessionTracking::default();
        if shell_family(&shell) == ShellFamily::Cmd {
            track.boot_echo = crate::pty::cmd_bootstrap_echo(&prompt_cmd);
            track.boot_echo_until = Some(std::time::Instant::now() + BOOT_ECHO_WINDOW);
        }
        let reader_width = track.width.clone();
        let record = SessionRecord {
            id: id.clone(),
            label: label.clone(),
            cwd: cwd.clone(),
            shell: shell.clone(),
            status: "active".to_string(),
            pty_pair: pair,
            writer,
            pending_execution_id: None,
            exec_state: SessionExecState::Booting,
            boot_prompt_received: false,
            command_sent_at: None,
            marker_nonce: nonce,
            marker_gen: 0,
            track,
            child: Some(child),
            created_at: now.clone(),
            last_active_at: now.clone(),
        };

        let summary = SessionSummary {
            id: id.clone(),
            label,
            cwd,
            shell: shell.clone(),
            status: "active".to_string(),
            created_at: now.clone(),
            last_active_at: now,
        };

        // The Booting record must be visible before the reader starts and
        // before the bootstrap write, or an early marker is handled with no session.
        {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
            registry.insert(record);
        }

        let session_id_for_reader = id.clone();
        let state_sessions = self.sessions.clone();
        let sink = self.event_sink.clone();
        let exit_sink = self.event_sink.clone();
        let exit_sessions = self.sessions.clone();
        let exit_id = id.clone();
        spawn_reader_with_exit(
            reader,
            reader_width,
            move |event| {
                Self::process_reader_event(&sink, &state_sessions, &session_id_for_reader, event);
            },
            move || Self::handle_session_exit(&exit_sink, &exit_sessions, &exit_id),
        );

        // On Windows the ConPTY output pipe can stay open after the shell
        // exits, so EOF alone is not enough: also watch the child itself.
        Self::spawn_child_watcher(self.event_sink.clone(), self.sessions.clone(), id.clone());

        if typed_bootstrap {
            persist_bootstrap(
                &self.sessions,
                &id,
                &bootstrap_writer,
                &prompt_cmd,
                &shell,
            )?;
        }

        Ok(summary)
    }

    pub fn list(&self) -> Result<Vec<SessionSummary>, String> {
        let registry = self.sessions.lock().map_err(|e| e.to_string())?;
        let summaries = registry
            .list()
            .iter()
            .map(|r| SessionSummary {
                id: r.id.clone(),
                label: r.label.clone(),
                cwd: r.cwd.clone(),
                shell: r.shell.clone(),
                status: r.status.clone(),
                created_at: r.created_at.clone(),
                last_active_at: r.last_active_at.clone(),
            })
            .collect();
        Ok(summaries)
    }

    pub fn close(&self, session_id: &str) -> Result<(), String> {
        let mut record = {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
            registry
                .remove(session_id)
                .ok_or_else(|| format!("Session not found: {session_id}"))?
        };
        // Kill and reap the shell off the registry lock. Closing the PTY alone
        // leaves backgrounded children running and, on Unix, a zombie shell.
        if let Some(mut child) = record.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    /// Poll the shell's child handle (`try_wait`, never blocking) and run the
    /// exit handling when the process ends. Stops when the session is closed
    /// or already marked exited by the reader's EOF path.
    pub(crate) fn spawn_child_watcher(
        sink: Arc<dyn RuntimeEventSink>,
        sessions: Arc<Mutex<SessionRegistry>>,
        session_id: String,
    ) {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let ended = {
                let Ok(mut reg) = sessions.lock() else { return };
                let Some(record) = reg.get_mut(&session_id) else {
                    return;
                };
                if record.status == "exited" {
                    return;
                }
                match record.child.as_mut() {
                    Some(child) => matches!(child.try_wait(), Ok(Some(_))),
                    None => return,
                }
            };
            if ended {
                Self::handle_session_exit(&sink, &sessions, &session_id);
                return;
            }
        });
    }

    /// The PTY stream ended, so the shell is gone (typed exit, crash, kill).
    /// Mark the session dead, fail any in-flight execution and tell the UI.
    pub(crate) fn handle_session_exit(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
    ) {
        let pending = {
            let Ok(mut reg) = sessions.lock() else { return };
            let Some(record) = reg.get_mut(session_id) else {
                return;
            };
            if record.status == "exited" {
                return;
            }
            record.status = "exited".to_string();
            record.exec_state = SessionExecState::Desynced;
            record.command_sent_at = None;
            record.pending_execution_id.take()
        };
        let now = chrono::Utc::now().to_rfc3339();
        sink.emit(RuntimeEvent::TerminalLine(TerminalLineEvent {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            execution_id: pending.clone(),
            // A runtime notice, not shell output: Raw Play must not write it
            // into a full-screen app's screen (apps/console skips "notice").
            kind: "notice".to_string(),
            text: "\nThe shell exited. Open a new session to continue.\n".to_string(),
            timestamp: now.clone(),
        }));
        if let Some(exec_id) = pending {
            sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
                execution_id: exec_id,
                session_id: session_id.to_string(),
                exit_code: 1,
                finished_at: now.clone(),
                status: "failure".to_string(),
                // The number is not a shell exit code. The UI uses the reason.
                exit_known: false,
                reason: Some("shell_exited".to_string()),
            }));
        }
        sink.emit(RuntimeEvent::SessionExecStateChanged(
            SessionExecStateChangedEvent {
                session_id: session_id.to_string(),
                exec_state: SessionExecState::Desynced.to_string(),
                changed_at: now,
            },
        ));
        eprintln!("[session] {session_id} shell exited");
    }

    pub fn update_cwd(&self, session_id: &str, cwd: &str) -> Result<(), String> {
        if cwd.is_empty() {
            return Err("cwd cannot be empty".to_string());
        }
        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
        let record = registry
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;
        record.cwd = cwd.to_string();
        Ok(())
    }

    /// What the reader hands on, in stream order: shell output, and the shell's
    /// markers (invisible OSC sequences the reader already took out of the
    /// text). Markers are the only thing that moves the session's state;
    /// nothing here looks at lines, rows or the console width.
    pub(crate) fn process_reader_event(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
        event: ReaderEvent,
    ) {
        match event {
            ReaderEvent::Text(text) => Self::process_text(sink, sessions, session_id, &text),
            ReaderEvent::Marker(marker) => Self::process_marker(sink, sessions, session_id, marker),
            ReaderEvent::Idle => Self::process_idle(sink, sessions, session_id),
        }
    }

    /// Feed raw reader text through a marker scanner into the state machine.
    /// For tests, which have no pty reader in front of them.
    #[cfg(test)]
    pub(crate) fn process_reader_chunk(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
        text: &str,
    ) {
        let mut scanner = crate::pty::MarkerScanner::default();
        for event in scanner.push(text) {
            Self::process_reader_event(sink, sessions, session_id, event);
        }
    }

    fn emit_stdout(
        sink: &Arc<dyn RuntimeEventSink>,
        session_id: &str,
        execution_id: Option<String>,
        text: String,
    ) {
        if text.is_empty() {
            return;
        }
        sink.emit(RuntimeEvent::TerminalLine(TerminalLineEvent {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            execution_id,
            kind: "stdout".to_string(),
            text,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }));
    }

    /// Shell output. It is shown as it arrives: there is no marker text in it
    /// to hold back. The one thing taken out is the echo of cmd's plumbing (the
    /// console echoes the line that was typed), across read boundaries.
    pub(crate) fn process_text(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
        text: &str,
    ) {
        let (exec_id, shown) = match sessions.lock() {
            Ok(mut reg) => match reg.get_mut(session_id) {
                Some(record) => {
                    let exec_id = record.pending_execution_id.clone();
                    let shown = if shell_family(&record.shell) == ShellFamily::Cmd {
                        // Always stripped, and a possible start of plumbing is
                        // always held (at most a few characters), whether or
                        // not a command is pending: the console paints an
                        // echo late and in pieces, and a redraw can repeat
                        // one after the prompt. The hold is released by the
                        // next text or by `process_idle`.
                        let t = &mut record.track;
                        if t.boot_echo_until.is_some_and(|until| std::time::Instant::now() > until) {
                            t.boot_echo.clear();
                            t.boot_echo_until = None;
                        }
                        strip_cmd_echo(&mut t.boot_echo, &mut t.display_hold, text, false)
                    } else {
                        text.to_string()
                    };
                    (exec_id, shown)
                }
                None => (None, text.to_string()),
            },
            Err(_) => (None, text.to_string()),
        };
        Self::emit_stdout(sink, session_id, exec_id, shown);
    }

    /// The stream went quiet: what was held back as a possible start of cmd
    /// plumbing was ordinary text after all.
    pub(crate) fn process_idle(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
    ) {
        let (exec_id, held) = match sessions.lock() {
            Ok(mut reg) => match reg.get_mut(session_id) {
                Some(record) if !record.track.display_hold.is_empty() => (
                    record.pending_execution_id.clone(),
                    {
                        let t = &mut record.track;
                        if t.boot_echo_until.is_some_and(|until| std::time::Instant::now() > until) {
                            t.boot_echo.clear();
                            t.boot_echo_until = None;
                        }
                        // A long partial line of the bootstrap echo is the start
                        // of that echo, not text: wait for the rest of it.
                        if !t.boot_echo.is_empty() && t.display_hold.len() >= 4 {
                            return;
                        }
                        strip_cmd_echo(&mut t.boot_echo, &mut t.display_hold, "", true)
                    },
                ),
                _ => return,
            },
            Err(_) => return,
        };
        Self::emit_stdout(sink, session_id, exec_id, held);
    }

    /// A marker from the shell's prompt hook.
    ///
    /// * `X` (cmd only) records the exit code of the command that just ran.
    /// * `P` says the shell is idle at a prompt. It finishes the pending
    ///   execution (with the exit code it carries, or the one `X` recorded),
    ///   returns the session to Ready, and reports the cwd.
    ///
    /// A cmd prompt that arrives with no exit code at all for a command the
    /// runtime ran means the command line could not carry its tail: when the
    /// tail was left off on purpose, the runtime writes it as a line of its
    /// own now that the shell is idle (once); when it was chained and never
    /// ran, the line did not parse and the command failed.
    ///
    /// A pasted block of N lines draws N prompts: a session the user was typing
    /// into returns to Ready only on the last one (`user_prompts_owed`), so an
    /// earlier prompt cannot finish an approved command written after the
    /// paste.
    pub(crate) fn process_marker(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
        marker: Marker,
    ) {
        struct Applied {
            was_boot: bool,
            pending: Option<String>,
            was_int: bool,
            exit_code: i32,
            exit_known: bool,
            reason: Option<String>,
            cwd: String,
            becomes_ready: bool,
        }
        enum Decision {
            Probe(PtyHandle, String),
            Applied(Applied),
        }

        // A missing record must not be reported as Ready.
        let decision = {
            let Ok(mut reg) = sessions.lock() else { return };
            let Some(record) = reg.get_mut(session_id) else { return };
            // The nonce filters stale or unrelated sequences; it does not
            // authenticate the sender (see the module docs of `pty`).
            if marker.nonce != record.marker_nonce {
                return;
            }
            match marker.kind {
                MarkerKind::Exit => {
                    if record.pending_execution_id.is_some() {
                        record.track.pending_exit = marker.exit;
                    }
                    return;
                }
                MarkerKind::Prompt => {}
            }
            let cwd = marker.cwd.clone();
            // A literal ~ cwd is not a location, and an empty one is not one either.
            if cwd.is_empty() || cwd == "~" || cwd.starts_with("~/") || cwd.starts_with("~\\") {
                return;
            }
            if let Some(chord) = marker.chord {
                record.track.clear_chord = chord;
            }
            let was_boot = !record.boot_prompt_received;
            let was_int = record.exec_state == SessionExecState::Interrupting;
            let mut exit = marker.exit.or(record.track.pending_exit);
            let mut invented = false;
            let mut decision = None;
            if record.pending_execution_id.is_some() && exit.is_none() && !was_int {
                let is_cmd = shell_family(&record.shell) == ShellFamily::Cmd;
                if is_cmd && record.track.pending_tail == CmdTail::None && !record.track.probed {
                    record.track.probed = true;
                    decision = Some(Decision::Probe(
                        record.writer.clone(),
                        cmd_probe_line(record.track.clear_chord),
                    ));
                } else {
                    // The shell drew a prompt and never reported a code. Keep the
                    // numeric placeholder internally; tell the UI the code is unknown.
                    exit = Some(UNKNOWN_EXIT_CODE);
                    invented = true;
                }
            }
            match decision {
                Some(decision) => decision,
                None => {
                    record.cwd = cwd.clone();
                    // Same lock as Ready. take() drops only the id that just finished.
                    let pending = record.pending_execution_id.take();
                    record.track.pending_exit = None;
                    record.track.pending_tail = CmdTail::Chained;
                    record.track.probed = false;
                    record.marker_gen = record.marker_gen.wrapping_add(1);
                    if was_boot {
                        record.boot_prompt_received = true;
                    }
                    let still_typed_into =
                        record.exec_state == SessionExecState::UserRunning && record.track.user_prompts_owed > 1;
                    if still_typed_into {
                        record.track.user_prompts_owed -= 1;
                    } else {
                        record.track.user_prompts_owed = 0;
                        record.exec_state = SessionExecState::Ready;
                    }
                    record.command_sent_at = None;
                    let (exit_code, exit_known, reason) = match exit {
                        Some(code) if invented => (code, false, Some("exit_unknown".to_string())),
                        Some(code) => (code, true, None),
                        None if was_int => (INTERRUPTED_EXIT_CODE, true, None),
                        None => (UNKNOWN_EXIT_CODE, false, Some("exit_unknown".to_string())),
                    };
                    Decision::Applied(Applied {
                        was_boot,
                        pending,
                        was_int,
                        exit_code,
                        exit_known,
                        reason,
                        cwd,
                        becomes_ready: !still_typed_into,
                    })
                }
            }
        };

        let applied = match decision {
            Decision::Probe(writer, line) => {
                // Off the registry lock: a pty write can block.
                if let Err(err) = write_raw(&writer, &line) {
                    eprintln!("[session] {session_id} cmd exit-code probe failed: {err}");
                }
                return;
            }
            Decision::Applied(applied) => applied,
        };

        sink.emit(RuntimeEvent::SessionCwdChanged(SessionCwdChangedEvent {
            session_id: session_id.to_string(),
            cwd: applied.cwd.clone(),
        }));

        if applied.was_boot {
            sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
                session_id: session_id.to_string(),
                cwd: applied.cwd.clone(),
            }));
            eprintln!("[session] {} ready (cwd: {})", session_id, applied.cwd);
        }

        if let Some(exec_id) = applied.pending {
            let status = if applied.was_int {
                "interrupted"
            } else if !applied.exit_known {
                "unknown"
            } else if applied.exit_code == 0 {
                "success"
            } else {
                "failure"
            };
            sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
                execution_id: exec_id,
                session_id: session_id.to_string(),
                exit_code: applied.exit_code,
                finished_at: chrono::Utc::now().to_rfc3339(),
                status: status.to_string(),
                exit_known: applied.exit_known,
                reason: applied.reason,
            }));
        }

        if applied.becomes_ready {
            sink.emit(RuntimeEvent::SessionExecStateChanged(
                SessionExecStateChangedEvent {
                    session_id: session_id.to_string(),
                    exec_state: SessionExecState::Ready.to_string(),
                    changed_at: chrono::Utc::now().to_rfc3339(),
                },
            ));
        }
    }
}

/// How long after a cmd session starts the console may still be painting the
/// echo of the bootstrap lines that were typed into it.
const BOOT_ECHO_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

fn persist_bootstrap(
    sessions: &Arc<Mutex<SessionRegistry>>,
    session_id: &str,
    writer: &PtyHandle,
    prompt_cmd: &str,
    shell: &str,
) -> Result<(), String> {
    if let Err(e) = write_raw(writer, prompt_cmd) {
        let removed = match sessions.lock() {
            Ok(mut registry) => registry.remove(session_id),
            Err(_) => None,
        };
        // Kill and reap the shell off the registry lock, as close() does.
        if let Some(mut record) = removed {
            if let Some(mut child) = record.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        eprintln!("[session] bootstrap write failed for shell {shell}: {e}");
        return Err(format!("bootstrap write failed: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CollectingSink;
    use crate::pty::marker_osc;

    const NONCE: &str = "test-nonce";

    fn prompt(exit: Option<i32>, cwd: &str) -> String {
        marker_osc('P', NONCE, exit, None, cwd)
    }

    fn exit_marker(code: i32) -> String {
        marker_osc('X', NONCE, Some(code), None, "")
    }

    fn booted_record(id: &str, nonce: &str) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            label: "Test".to_string(),
            cwd: "/tmp".to_string(),
            shell: "bash".to_string(),
            status: "active".to_string(),
            pty_pair: make_dummy_pty_pair(),
            writer: make_dummy_writer(),
            pending_execution_id: None,
            exec_state: SessionExecState::Ready,
            boot_prompt_received: true,
            command_sent_at: None,
            marker_nonce: nonce.to_string(),
            marker_gen: 0,
            track: SessionTracking::default(),
            child: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_active_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn setup() -> (Arc<CollectingSink>, Arc<dyn RuntimeEventSink>, Arc<Mutex<SessionRegistry>>) {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        (sink, sink_dyn, Arc::new(Mutex::new(SessionRegistry::new())))
    }

    fn running(sessions: &Arc<Mutex<SessionRegistry>>, shell: &str, exec: &str) {
        let mut record = booted_record("s1", NONCE);
        record.shell = shell.to_string();
        record.pending_execution_id = Some(exec.to_string());
        record.exec_state = SessionExecState::Running;
        sessions.lock().unwrap().insert(record);
    }

    fn finished(sink: &CollectingSink) -> Vec<(String, i32, String)> {
        sink.events()
            .into_iter()
            .filter_map(|e| match e {
                RuntimeEvent::ExecutionFinished(f) => Some((f.execution_id, f.exit_code, f.status)),
                _ => None,
            })
            .collect()
    }

    fn shown_text(sink: &CollectingSink) -> String {
        sink.events()
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(t) if t.kind == "stdout" => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    }

    fn state_of(sessions: &Arc<Mutex<SessionRegistry>>) -> SessionExecState {
        sessions.lock().unwrap().get("s1").unwrap().exec_state.clone()
    }

    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // ---- Text ----

    #[test]
    fn test_process_reader_chunk_plain_text() {
        let (sink, sink_dyn, sessions) = setup();
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "test-session", "hello world\n");
        assert_eq!(sink.len(), 1);
        match &sink.events()[0] {
            RuntimeEvent::TerminalLine(e) => {
                assert_eq!(e.session_id, "test-session");
                assert_eq!(e.text, "hello world\n");
                assert_eq!(e.kind, "stdout");
            }
            _ => panic!("expected TerminalLine event"),
        }
    }

    #[test]
    fn output_is_shown_at_once_with_no_line_buffering_and_never_shows_a_marker() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "e1");
        // An unterminated tail (a prompt, a progress bar) is shown as it arrives.
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "progress 50%");
        assert_eq!(shown_text(&sink), "progress 50%");
        // Output, the marker glued to unterminated output, then more output: the
        // marker is never text, and what surrounds it is shown in order.
        let chunk = format!("\rdone{}$ ", prompt(Some(0), "/tmp"));
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &chunk);
        assert_eq!(shown_text(&sink), "progress 50%\rdone$ ");
        assert!(!shown_text(&sink).contains("7733"));
        assert_eq!(finished(&sink), vec![("e1".to_string(), 0, "success".to_string())]);
        // Output precedes the finish in the event stream.
        let events = sink.events();
        let text_at = events
            .iter()
            .position(|e| matches!(e, RuntimeEvent::TerminalLine(t) if t.text == "\rdone"))
            .expect("the output before the marker is shown");
        let finish_at = events.iter().position(|e| matches!(e, RuntimeEvent::ExecutionFinished(_))).unwrap();
        assert!(text_at < finish_at);
    }

    // ---- Boot, completion, exit codes ----

    #[test]
    fn test_process_reader_chunk_prompt_marker_boot() {
        let (sink, sink_dyn, sessions) = setup();
        let mut record = booted_record("s1", NONCE);
        record.cwd = "/old".to_string();
        record.exec_state = SessionExecState::Booting;
        record.boot_prompt_received = false;
        sessions.lock().unwrap().insert(record);

        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/home/user"));

        let events = sink.events();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], RuntimeEvent::SessionCwdChanged(_)));
        assert!(matches!(events[1], RuntimeEvent::SessionReady(_)));
        assert!(matches!(events[2], RuntimeEvent::SessionExecStateChanged(_)));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.cwd, "/home/user");
        assert!(record.boot_prompt_received);
        assert_eq!(record.exec_state, SessionExecState::Ready);
    }

    #[test]
    fn test_process_reader_chunk_execution_completion() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "exec-1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/tmp"));
        let events = sink.events();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], RuntimeEvent::SessionCwdChanged(_)));
        match &events[1] {
            RuntimeEvent::ExecutionFinished(e) => {
                assert_eq!(e.execution_id, "exec-1");
                assert_eq!(e.exit_code, 0);
                assert_eq!(e.status, "success");
            }
            _ => panic!("expected ExecutionFinished event"),
        }
        assert!(matches!(events[2], RuntimeEvent::SessionExecStateChanged(_)));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert!(record.pending_execution_id.is_none());
        assert_eq!(record.exec_state, SessionExecState::Ready);
    }

    #[test]
    fn matching_nonce_finishes_with_real_exit_and_cwd() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "exec-1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(7), "/var/work"));
        assert_eq!(finished(&sink), vec![("exec-1".to_string(), 7, "failure".to_string())]);
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.cwd, "/var/work");
        assert!(record.pending_execution_id.is_none());
        assert_eq!(record.exec_state, SessionExecState::Ready);
        assert_eq!(record.marker_gen, 1);
    }

    #[test]
    fn test_process_reader_chunk_interrupt_completion() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "exec-1");
        sessions.lock().unwrap().get_mut("s1").unwrap().exec_state = SessionExecState::Interrupting;
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(130), "/tmp"));
        assert_eq!(finished(&sink), vec![("exec-1".to_string(), 130, "interrupted".to_string())]);
        assert_eq!(state_of(&sessions), SessionExecState::Ready);
    }

    #[test]
    fn a_marker_with_a_300_character_cwd_finishes_the_command() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "e1");
        let cwd = format!("/{}", "long-directory-name/".repeat(16));
        assert!(cwd.len() > 300);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), &cwd));
        assert_eq!(finished(&sink), vec![("e1".to_string(), 0, "success".to_string())]);
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().cwd, cwd);
    }

    #[test]
    fn a_cwd_with_separators_and_line_breaks_round_trips() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "e1");
        let cwd = "/work/a;b|c%d\ne";
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), cwd));
        assert_eq!(finished(&sink).len(), 1);
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().cwd, cwd);
        let _ = sink;
    }

    // ---- What is not a marker ----

    #[test]
    fn markers_for_another_session_or_without_a_session_do_nothing() {
        let (sink, sink_dyn, sessions) = setup();
        // No session: nothing is reported as Ready.
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/tmp"));
        assert!(sink.events().iter().all(|e| !matches!(e, RuntimeEvent::SessionReady(_) | RuntimeEvent::SessionExecStateChanged(_))));
        // A session with another nonce: stale or unrelated sequences.
        running(&sessions, "bash", "e1");
        let stale = marker_osc('P', "other-nonce", Some(0), None, "/tmp");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &stale);
        let stale_exit = marker_osc('X', "other-nonce", Some(5), None, "");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &stale_exit);
        assert!(finished(&sink).is_empty());
        assert_eq!(state_of(&sessions), SessionExecState::Running);
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().track.pending_exit, None);
        // And neither was shown.
        assert!(!shown_text(&sink).contains("other-nonce"));
    }

    #[test]
    fn the_public_marker_shape_without_the_session_nonce_finishes_nothing() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "e1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "\x1b]7733;P;;0;;/tmp\x07");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "\x1b]7733;P;/tmp;0\x07");
        assert!(finished(&sink).is_empty());
        assert_eq!(state_of(&sessions), SessionExecState::Running);
    }

    #[test]
    fn tilde_and_empty_cwds_are_not_stored_and_do_not_finish() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "exec-1");
        for cwd in ["~", "~/work", "~\\work", ""] {
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), cwd));
        }
        assert!(finished(&sink).is_empty());
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.cwd, "/tmp");
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-1"));
        assert_eq!(record.exec_state, SessionExecState::Running);
    }

    #[test]
    fn a_bad_exit_code_is_not_a_marker_and_finishes_nothing() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "exec-1");
        for raw in ["\x1b]7733;P;test-nonce;nope;;/tmp\x07", "\x1b]7733;P;test-nonce\x07"] {
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", raw);
        }
        assert!(finished(&sink).is_empty());
        assert!(shown_text(&sink).is_empty(), "{:?}", shown_text(&sink));
        assert_eq!(state_of(&sessions), SessionExecState::Running);
    }

    #[test]
    fn reading_the_shell_prompt_state_is_not_a_secret_but_is_documented() {
        // The nonce is shell-readable state: a command in the session that
        // knows it can finish itself. That is the documented residual risk.
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "bash", "e1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/chosen"));
        assert_eq!(finished(&sink).len(), 1);
        let src = include_str!("../pty.rs");
        assert!(src.contains("Residual risk"));
    }

    #[test]
    fn prompt_clear_does_not_wipe_a_newer_pending_id() {
        let inner = Arc::new(CollectingSink::new());
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        running(&sessions, "bash", "exec-old");
        let sink = ReenterOnFinish { inner: inner.clone(), sessions: sessions.clone() };
        let sink_dyn: Arc<dyn RuntimeEventSink> = Arc::new(sink);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/var/work"));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-new"));
        assert_eq!(record.exec_state, SessionExecState::Running);
    }

    // ---- cmd: the exit code comes in a marker of its own ----

    #[test]
    fn cmd_exit_marker_then_prompt_finishes_with_the_reported_code() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "cmd.exe", "e1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &exit_marker(7));
        // Nothing finishes on the exit marker alone.
        assert!(finished(&sink).is_empty());
        assert_eq!(state_of(&sessions), SessionExecState::Running);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        assert_eq!(finished(&sink), vec![("e1".to_string(), 7, "failure".to_string())]);
        assert_eq!(state_of(&sessions), SessionExecState::Ready);
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().track.pending_exit, None);
    }

    #[test]
    fn an_exit_marker_with_nothing_pending_is_ignored_and_does_not_leak_into_the_next_command() {
        let (sink, sink_dyn, sessions) = setup();
        sessions.lock().unwrap().insert({
            let mut r = booted_record("s1", NONCE);
            r.shell = "cmd.exe".to_string();
            r
        });
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &exit_marker(9));
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().track.pending_exit, None);
        sessions.lock().unwrap().get_mut("s1").unwrap().pending_execution_id = Some("e1".into());
        sessions.lock().unwrap().get_mut("s1").unwrap().exec_state = SessionExecState::Running;
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &exit_marker(0));
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        assert_eq!(finished(&sink), vec![("e1".to_string(), 0, "success".to_string())]);
    }

    #[test]
    fn a_cmd_prompt_without_an_exit_for_a_chained_line_means_it_never_ran() {
        // `echo a & & tail` does not parse: cmd prints an error and draws a
        // prompt. The execution must not stay pending.
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "cmd.exe", "e1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        assert_eq!(
            finished(&sink),
            vec![("e1".to_string(), UNKNOWN_EXIT_CODE, "unknown".to_string())]
        );
        let unknown = sink.events().into_iter().find_map(|event| match event {
            RuntimeEvent::ExecutionFinished(finished) => Some(finished),
            _ => None,
        }).expect("finished");
        assert!(!unknown.exit_known);
        assert_eq!(unknown.reason.as_deref(), Some("exit_unknown"));
        assert_eq!(state_of(&sessions), SessionExecState::Ready);
    }

    #[test]
    fn a_cmd_prompt_without_an_exit_for_a_line_without_a_tail_sends_the_probe_once() {
        let (sink, sink_dyn, sessions) = setup();
        let bytes = Arc::new(Mutex::new(Vec::new()));
        running(&sessions, "cmd.exe", "e1");
        {
            let mut reg = sessions.lock().unwrap();
            let record = reg.get_mut("s1").unwrap();
            record.track.pending_tail = CmdTail::None;
            record.writer = Arc::new(Mutex::new(Box::new(Capture(bytes.clone())) as Box<dyn std::io::Write + Send>));
        }
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        // Still running: the probe was written, the shell is idle but the code is not known.
        assert!(finished(&sink).is_empty());
        assert_eq!(state_of(&sessions), SessionExecState::Running);
        assert_eq!(String::from_utf8(bytes.lock().unwrap().clone()).unwrap(), cmd_probe_line(true));
        // The probe answers: exit marker, then its prompt.
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &exit_marker(0));
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        assert_eq!(finished(&sink), vec![("e1".to_string(), 0, "success".to_string())]);
        assert_eq!(bytes.lock().unwrap().len(), cmd_probe_line(true).len(), "the probe is sent once");
        // A probe that is not answered does not hang the command either.
        let (sink2, sink2_dyn, sessions2) = setup();
        running(&sessions2, "cmd.exe", "e2");
        sessions2.lock().unwrap().get_mut("s1").unwrap().track.pending_tail = CmdTail::None;
        SessionService::process_reader_chunk(&sink2_dyn, &sessions2, "s1", &prompt(None, "C:\\w"));
        SessionService::process_reader_chunk(&sink2_dyn, &sessions2, "s1", &prompt(None, "C:\\w"));
        assert_eq!(finished(&sink2), vec![("e2".to_string(), UNKNOWN_EXIT_CODE, "unknown".to_string())]);
        let unknown = sink2.events().into_iter().find_map(|e| match e {
            RuntimeEvent::ExecutionFinished(f) => Some(f),
            _ => None,
        }).expect("finished");
        assert!(!unknown.exit_known);
        assert_eq!(unknown.reason.as_deref(), Some("exit_unknown"));
    }

    #[test]
    fn a_cmd_prompt_after_ctrl_c_finishes_the_command_as_interrupted() {
        // Ctrl+C drops the rest of a cmd line, the tail included.
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "cmd.exe", "e1");
        sessions.lock().unwrap().get_mut("s1").unwrap().exec_state = SessionExecState::Interrupting;
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        assert_eq!(finished(&sink), vec![("e1".to_string(), 130, "interrupted".to_string())]);
    }

    #[test]
    fn a_cmd_prompt_returns_a_session_booting_or_user_running_to_ready() {
        for state in [SessionExecState::UserRunning, SessionExecState::Booting, SessionExecState::Desynced] {
            let (sink, sink_dyn, sessions) = setup();
            let mut record = booted_record("s1", NONCE);
            record.shell = "cmd.exe".to_string();
            record.exec_state = state;
            sessions.lock().unwrap().insert(record);
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
            assert_eq!(state_of(&sessions), SessionExecState::Ready);
            assert!(sink.events().iter().any(
                |e| matches!(e, RuntimeEvent::SessionExecStateChanged(s) if s.exec_state == "ready")
            ));
        }
    }

    #[test]
    fn cmd_plumbing_in_the_echo_is_never_shown_even_when_a_read_splits_it() {
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "cmd.exe", "e1");
        // The echo of an approved line, in pieces (observed live: `%__` then `cuz% & dir /b`).
        for piece in ["C:\\w>%__", "cuz% & dir /b", " & %_", "_cui%", "\r\nfile.txt\r\nC:\\w>%__cui%"] {
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", piece);
        }
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &exit_marker(0));
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(None, "C:\\w"));
        let shown = shown_text(&sink);
        assert_eq!(shown, "C:\\w>dir /b\r\nfile.txt\r\nC:\\w>", "{shown:?}");
        assert!(!shown.contains("__cu"));
        // What was held at the end is released when the stream goes quiet, not lost.
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "cmd.exe", "e1");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "done 50%");
        assert_eq!(shown_text(&sink), "done 50");
        SessionService::process_reader_event(&sink_dyn, &sessions, "s1", ReaderEvent::Idle);
        assert_eq!(shown_text(&sink), "done 50%");
    }

    #[test]
    fn cmd_plumbing_painted_late_or_repeated_with_no_command_pending_is_never_shown() {
        // The console can paint an echo after the prompt marker (the command
        // is no longer pending) or redraw it; a read can cut it anywhere.
        let echo = "C:\\w>%__cuz% & echo hi & %__cui%\x1b[K\r\nhi\r\n";
        for cut in 0..=echo.len() {
            let (sink, sink_dyn, sessions) = setup();
            let mut record = booted_record("s1", NONCE);
            record.shell = "cmd.exe".to_string();
            sessions.lock().unwrap().insert(record);
            let mut scanner = crate::pty::MarkerScanner::default();
            for part in [&echo[..cut], &echo[cut..]] {
                for event in scanner.push(part) {
                    SessionService::process_reader_event(&sink_dyn, &sessions, "s1", event);
                }
            }
            SessionService::process_reader_event(&sink_dyn, &sessions, "s1", ReaderEvent::Idle);
            assert_eq!(shown_text(&sink), "C:\\w>echo hi\x1b[K\r\nhi\r\n", "cut at {cut}");
        }
    }

    // ---- Typed and pasted input ----

    #[test]
    fn a_prompt_returns_a_user_running_session_to_ready() {
        let (sink, sink_dyn, sessions) = setup();
        let mut record = booted_record("s1", NONCE);
        record.exec_state = SessionExecState::UserRunning;
        record.track.user_prompts_owed = 1;
        sessions.lock().unwrap().insert(record);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/tmp"));
        assert_eq!(state_of(&sessions), SessionExecState::Ready);
        assert!(sink.events().iter().any(
            |e| matches!(e, RuntimeEvent::SessionExecStateChanged(s) if s.exec_state == "ready")
        ));
    }

    #[test]
    fn a_pasted_block_of_two_lines_needs_two_prompts_to_return_to_ready() {
        let (sink, sink_dyn, sessions) = setup();
        let mut record = booted_record("s1", NONCE);
        record.exec_state = SessionExecState::UserRunning;
        record.track.user_prompts_owed = 2;
        sessions.lock().unwrap().insert(record);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/a"));
        // The first prompt: still running the second line; the cwd is reported
        // but Ready is not.
        assert_eq!(state_of(&sessions), SessionExecState::UserRunning);
        assert!(sink.events().iter().all(
            |e| !matches!(e, RuntimeEvent::SessionExecStateChanged(s) if s.exec_state == "ready")
        ));
        assert!(sink.events().iter().any(|e| matches!(e, RuntimeEvent::SessionCwdChanged(c) if c.cwd == "/a")));
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/b"));
        assert_eq!(state_of(&sessions), SessionExecState::Ready);
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().track.user_prompts_owed, 0);
    }

    #[test]
    fn the_clear_chord_state_follows_what_the_shell_reports() {
        let (_sink, sink_dyn, sessions) = setup();
        sessions.lock().unwrap().insert(booted_record("s1", NONCE));
        assert!(sessions.lock().unwrap().get("s1").unwrap().track.clear_chord);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker_osc('P', NONCE, Some(0), Some(false), "/tmp"));
        assert!(!sessions.lock().unwrap().get("s1").unwrap().track.clear_chord);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker_osc('P', NONCE, Some(0), Some(true), "/tmp"));
        assert!(sessions.lock().unwrap().get("s1").unwrap().track.clear_chord);
        // A marker that does not say leaves it alone.
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &prompt(Some(0), "/tmp"));
        assert!(sessions.lock().unwrap().get("s1").unwrap().track.clear_chord);
    }

    // ---- Shell exit ----

    #[test]
    fn shell_exit_fails_pending_execution_and_marks_session_dead() {
        let (sink, sink_dyn, sessions) = setup();
        insert_session(&sessions, "s1", SessionExecState::Running, true, Some("exec-1".to_string()));
        SessionService::handle_session_exit(&sink_dyn, &sessions, "s1");
        assert_eq!(finished(&sink), vec![("exec-1".to_string(), 1, "failure".to_string())]);
        let ended = sink.events().into_iter().find_map(|event| match event {
            RuntimeEvent::ExecutionFinished(finished) => Some(finished),
            _ => None,
        }).expect("finished");
        assert!(!ended.exit_known);
        assert_eq!(ended.reason.as_deref(), Some("shell_exited"));
        {
            let reg = sessions.lock().unwrap();
            let record = reg.get("s1").unwrap();
            assert_eq!(record.status, "exited");
            assert!(record.pending_execution_id.is_none());
        }
        // The exit text is a runtime notice, not shell output: Raw Play must
        // not write it into a full-screen app's screen.
        let lines: Vec<(String, String)> = sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(l) => Some((l.kind, l.text)),
                _ => None,
            })
            .collect();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].0, "notice");
        assert!(lines[0].1.to_ascii_lowercase().contains("the shell exited"), "{lines:?}");
        assert!(lines.iter().all(|(kind, _)| kind != "stdout"));
        // Idempotent: a second call emits nothing more.
        let n = sink.len();
        SessionService::handle_session_exit(&sink_dyn, &sessions, "s1");
        assert_eq!(sink.len(), n);
    }

    // ---- Through the real reader ----

    /// Run a scripted byte stream through the reader, into the state machine.
    fn run_stream(
        chunks: Vec<Vec<u8>>,
        sink_dyn: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
    ) {
        struct Chunks(Vec<Vec<u8>>);
        impl std::io::Read for Chunks {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                let next = self.0.remove(0);
                buf[..next.len()].copy_from_slice(&next);
                Ok(next.len())
            }
        }
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let sink = sink_dyn.clone();
        let sessions = sessions.clone();
        crate::pty::spawn_reader_with_exit(
            Chunks(chunks),
            Arc::new(std::sync::atomic::AtomicU16::new(crate::pty::PTY_COLS)),
            move |event| SessionService::process_reader_event(&sink, &sessions, "s1", event),
            move || {
                let _ = done_tx.send(());
            },
        );
        done_rx.recv_timeout(std::time::Duration::from_secs(5)).expect("the reader finished");
    }

    #[test]
    fn conpty_stream_without_line_breaks_still_finishes_the_command() {
        // The shape a ConPTY produced live: the output of `echo probe-ok` and
        // the prompt are separated by cursor positioning, not CR LF; the marker
        // is an OSC in between, cut in the middle by a read.
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "powershell.exe", "e1");
        let marker = prompt(Some(0), "C:\\work");
        let raw = format!("\x1b[?25h\x1b[mprobe-ok\x1b[?25l\x1b[15;1H{marker}\x1b[16;1H> ");
        let cut = raw.find("7733").unwrap() + 2;
        let chunks = vec![raw.as_bytes()[..cut].to_vec(), raw.as_bytes()[cut..].to_vec()];
        run_stream(chunks, &sink_dyn, &sessions);
        assert_eq!(finished(&sink), vec![("e1".to_string(), 0, "success".to_string())]);
        let shown = shown_text(&sink);
        assert!(shown.contains("probe-ok"), "{shown:?}");
        assert!(!shown.contains("7733") && !shown.contains("COMMANDUI"), "{shown:?}");
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().cwd, "C:\\work");
    }

    #[test]
    fn a_marker_row_longer_than_any_console_width_is_never_text() {
        // F-86921478: a 240-cell row used to be split by a soft wrap that was
        // not recognised, and the command hung. The marker is not a row.
        let (sink, sink_dyn, sessions) = setup();
        running(&sessions, "powershell.exe", "e1");
        let cwd = format!("C:\\{}", "x".repeat(300));
        let raw = format!("second\r\n{}\r\n> ", prompt(Some(0), &cwd));
        run_stream(vec![raw.into_bytes()], &sink_dyn, &sessions);
        assert_eq!(finished(&sink), vec![("e1".to_string(), 0, "success".to_string())]);
        assert!(!shown_text(&sink).contains(&"x".repeat(20)), "the cwd is not displayed");
    }

    // ---- Lifecycle ----

    #[test]
    fn create_rejects_unsupported_shell_without_inserting() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = SessionService::new(sessions.clone(), sink);
        let err = svc
            .create(CreateSessionRequest {
                label: Some("nope".to_string()),
                cwd: Some("/tmp".to_string()),
                shell: Some("fish".to_string()),
            })
            .unwrap_err();
        assert!(err.contains("unsupported"), "{err}");
        assert!(sessions.lock().unwrap().list().is_empty());
    }

    #[test]
    fn bootstrap_write_failure_removes_session() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Booting,
            false,
            None,
        );
        let writer = Arc::new(Mutex::new(
            Box::new(FailWrite) as Box<dyn std::io::Write + Send>,
        ));
        let err = persist_bootstrap(&sessions, "s1", &writer, "echo hi\n", "bash").unwrap_err();
        assert!(err.contains("bootstrap write failed"), "{err}");
        assert!(sessions.lock().unwrap().get("s1").is_none());
    }

    #[test]
    fn bootstrap_write_success_keeps_session() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Booting,
            false,
            None,
        );
        let writer = Arc::new(Mutex::new(
            Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>,
        ));
        persist_bootstrap(&sessions, "s1", &writer, "echo hi\n", "bash").unwrap();
        assert!(sessions.lock().unwrap().get("s1").is_some());
    }

    #[test]
    fn list_on_empty_registry_is_empty() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = SessionService::new(sessions, sink);
        let listed = svc.list().unwrap();
        assert!(listed.is_empty());
    }

    #[test]
    fn update_cwd_rejects_empty_and_missing_session() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = SessionService::new(sessions, sink);
        let empty = svc.update_cwd("missing", "").unwrap_err();
        assert!(empty.contains("cwd cannot be empty"), "{empty}");
        let missing = svc
            .update_cwd("missing", &std::env::temp_dir().to_string_lossy())
            .unwrap_err();
        assert!(missing.contains("Session not found"), "{missing}");
    }

    #[test]
    fn create_missing_bash_binary_does_not_insert() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = SessionService::new(sessions.clone(), sink);
        let cwd = std::env::temp_dir().to_string_lossy().to_string();
        let err = svc
            .create(CreateSessionRequest {
                label: Some("missing-bash".to_string()),
                cwd: Some(cwd),
                shell: Some("/nonexistent-dir/bash".to_string()),
            })
            .unwrap_err();
        assert!(
            err.contains("Failed to spawn shell") || err.contains("Failed to open PTY"),
            "{err}"
        );
        assert!(sessions.lock().unwrap().list().is_empty());
    }

    #[test]
    fn create_list_update_cwd_and_close_round_trip() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = SessionService::new(sessions.clone(), sink);
        let cwd = std::env::temp_dir().to_string_lossy().to_string();
        let summary = svc
            .create(CreateSessionRequest {
                label: Some("covered".to_string()),
                cwd: Some(cwd.clone()),
                shell: None,
            })
            .unwrap();
        let _close = CloseSession {
            svc: &svc,
            id: summary.id.clone(),
        };

        assert!(!summary.id.is_empty());
        assert_eq!(summary.label, "covered");
        assert_eq!(summary.cwd, cwd);
        assert_eq!(summary.shell, default_shell());
        assert_eq!(summary.status, "active");
        assert!(!summary.created_at.is_empty());
        assert!(!summary.last_active_at.is_empty());

        let listed = svc.list().unwrap();
        assert!(listed.iter().any(|item| item.id == summary.id));

        let updated = std::env::temp_dir()
            .join("commandui-cwd-marker")
            .to_string_lossy()
            .to_string();
        svc.update_cwd(&summary.id, &updated).unwrap();
        {
            let reg = sessions.lock().unwrap();
            assert_eq!(reg.get(&summary.id).unwrap().cwd, updated);
        }
        assert!(svc
            .list()
            .unwrap()
            .iter()
            .any(|item| item.id == summary.id && item.cwd == updated));

        svc.close(&summary.id).unwrap();
        assert!(sessions.lock().unwrap().get(&summary.id).is_none());
        assert!(svc.list().unwrap().iter().all(|item| item.id != summary.id));
        let err = svc.close(&summary.id).unwrap_err();
        assert!(err.contains("Session not found"), "{err}");
    }

    struct CloseSession<'a> {
        svc: &'a SessionService,
        id: String,
    }

    impl Drop for CloseSession<'_> {
        fn drop(&mut self) {
            let _ = self.svc.close(&self.id);
        }
    }

    struct ReenterOnFinish {
        inner: Arc<CollectingSink>,
        sessions: Arc<Mutex<SessionRegistry>>,
    }

    impl RuntimeEventSink for ReenterOnFinish {
        fn emit(&self, event: RuntimeEvent) {
            if let RuntimeEvent::ExecutionFinished(fin) = &event {
                assert_eq!(fin.execution_id, "exec-old");
                let mut reg = self.sessions.lock().unwrap();
                let record = reg.get_mut("s1").unwrap();
                record.pending_execution_id = Some("exec-new".to_string());
                record.exec_state = SessionExecState::Running;
            }
            self.inner.emit(event);
        }
    }

    struct FailWrite;

    impl std::io::Write for FailWrite {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("write failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // --- Test helpers ---

    fn insert_session(
        sessions: &Arc<Mutex<SessionRegistry>>,
        id: &str,
        exec_state: SessionExecState,
        booted: bool,
        pending: Option<String>,
    ) {
        let mut reg = sessions.lock().unwrap();
        reg.insert(SessionRecord {
            id: id.to_string(),
            label: "Test".to_string(),
            cwd: "/tmp".to_string(),
            shell: "bash".to_string(),
            status: "active".to_string(),
            pty_pair: make_dummy_pty_pair(),
            writer: make_dummy_writer(),
            pending_execution_id: pending,
            exec_state,
            boot_prompt_received: booted,
            command_sent_at: None,
            marker_nonce: "test-nonce".to_string(),
            marker_gen: 0,
            track: SessionTracking::default(),
            child: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_active_at: "2026-01-01T00:00:00Z".to_string(),
        });
    }

    fn make_dummy_pty_pair() -> portable_pty::PtyPair {
        let pty_system = portable_pty::native_pty_system();
        pty_system
            .openpty(portable_pty::PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("failed to open test pty")
    }

    fn make_dummy_writer() -> crate::pty::PtyHandle {
        Arc::new(Mutex::new(Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>))
    }
}
