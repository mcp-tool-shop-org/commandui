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
    bootstrap_prompt, clone_reader, default_shell, new_marker_nonce, spawn_reader_with_exit,
    spawn_shell,
    write_raw, PtyHandle, PROMPT_MARKER,
};
use crate::session::{SessionExecState, SessionRecord, SessionRegistry};
use std::sync::{Arc, Mutex};

/// Most bytes of an unfinished line the reader keeps for a session.
const MAX_READ_BUFFER: usize = 1024 * 1024;
/// Bytes kept from the end of an oversized line so a marker that follows can
/// still be parsed.
const READ_BUFFER_TAIL: usize = 4096;

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
        // Resolve the default cwd before anything is spawned. current_dir()
        // fails if the process directory was removed.
        let cwd = match request.cwd {
            Some(cwd) => cwd,
            None => std::env::current_dir()
                .map_err(|e| format!("Failed to resolve working directory: {e}"))?
                .to_string_lossy()
                .to_string(),
        };
        let now = chrono::Utc::now().to_rfc3339();
        let nonce = new_marker_nonce();
        let Some(prompt_cmd) = bootstrap_prompt(&shell, &nonce) else {
            eprintln!("[session] refused unsupported shell: {shell}");
            return Err(format!("unsupported shell: {shell}"));
        };

        let (pair, writer, child) = spawn_shell(&shell, Some(&cwd))?;
        let reader = clone_reader(&pair)?;
        let bootstrap_writer = writer.clone();

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
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
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
            move |text| {
                Self::process_reader_chunk(
                    &sink,
                    &state_sessions,
                    &session_id_for_reader,
                    &text,
                );
            },
            move || Self::handle_session_exit(&exit_sink, &exit_sessions, &exit_id),
        );

        // On Windows the ConPTY output pipe can stay open after the shell
        // exits, so EOF alone is not enough: also watch the child itself.
        Self::spawn_child_watcher(self.event_sink.clone(), self.sessions.clone(), id.clone());

        persist_bootstrap(
            &self.sessions,
            &id,
            &bootstrap_writer,
            &prompt_cmd,
            &shell,
        )?;

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
            kind: "stdout".to_string(),
            text: "\n[commandui: the shell exited. Open a new session to continue.]\n".to_string(),
            timestamp: now.clone(),
        }));
        if let Some(exec_id) = pending {
            sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
                execution_id: exec_id,
                session_id: session_id.to_string(),
                exit_code: 1,
                finished_at: now.clone(),
                status: "failure".to_string(),
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

    /// Reader-loop chunk processor.
    ///
    /// This is the state machine that was previously fused into the Tauri
    /// closure in session.rs. It owns:
    /// - prompt-marker parsing
    /// - cwd extraction
    /// - boot detection
    /// - execution completion inference
    /// - exec-state transitions
    /// - semantic event emission through the sink
    pub(crate) fn process_reader_chunk(
        sink: &Arc<dyn RuntimeEventSink>,
        sessions: &Arc<Mutex<SessionRegistry>>,
        session_id: &str,
        text: &str,
    ) {
        let mut display_text = String::new();

        // Carry a trailing partial line on the session for marker parsing.
        // The unterminated tail is still displayed at once (prompts, echo),
        // unless it could be the start of a marker line.
        let (current_exec_id, complete, skip, tail_display, nonce) = match sessions.lock() {
            Ok(mut reg) => {
                if let Some(record) = reg.get_mut(session_id) {
                    record.read_buffer.push_str(text);
                    let exec_id = record.pending_execution_id.clone();
                    let nonce = record.marker_nonce.clone();
                    let mut complete = drain_complete_lines(&mut record.read_buffer);
                    let skip = record.emitted_tail.min(complete.len());
                    record.emitted_tail = record.emitted_tail.saturating_sub(complete.len());
                    let before = record.read_buffer.len();
                    if let Some(notice) = truncate_read_buffer(&mut record.read_buffer) {
                        let cut = before - record.read_buffer.len();
                        record.emitted_tail = record.emitted_tail.saturating_sub(cut);
                        complete.push_str(&notice);
                    }
                    let tail_display = take_displayable_tail(record);
                    (exec_id, complete, skip, tail_display, Some(nonce))
                } else {
                    let mut scratch = text.to_string();
                    let complete = drain_complete_lines(&mut scratch);
                    (None, complete, 0, scratch, None)
                }
            }
            Err(_) => {
                let mut scratch = text.to_string();
                let complete = drain_complete_lines(&mut scratch);
                (None, complete, 0, scratch, None)
            }
        };

        // Bytes of the first complete line that were already shown as a tail.
        let mut line_start = 0usize;
        let mut prev_fresh = false;
        for line in complete.split_inclusive('\n') {
            let already = skip.saturating_sub(line_start).min(line.len());
            line_start += line.len();
            let shown_rest = &line[already..];
            let prev_line_fresh = prev_fresh;
            prev_fresh = already == 0;
            let Some(nonce) = nonce.as_deref() else {
                display_text.push_str(shown_rest);
                continue;
            };
            let Some(prompt) = parse_prompt_line(line, nonce) else {
                display_text.push_str(shown_rest);
                continue;
            };

            // A missing record must not be reported as Ready.
            let applied = if let Ok(mut reg) = sessions.lock() {
                if let Some(record) = reg.get_mut(session_id) {
                    record.cwd = prompt.cwd.clone();
                    let was_boot = !record.boot_prompt_received;
                    let was_int = record.exec_state == SessionExecState::Interrupting;
                    // Same lock as Ready. take() drops only the id that just finished.
                    let pending = record.pending_execution_id.take();
                    record.marker_gen = record.marker_gen.wrapping_add(1);
                    if was_boot {
                        record.boot_prompt_received = true;
                    }
                    record.exec_state = SessionExecState::Ready;
                    record.command_sent_at = None;
                    Some((was_boot, pending, was_int))
                } else {
                    None
                }
            } else {
                None
            };

            let Some((was_booting, pending_exec, was_interrupting)) = applied else {
                display_text.push_str(shown_rest);
                continue;
            };

            // The marker is written after a newline of its own, which shows
            // up as a blank line right before it. Hide that one line when it
            // is still in this chunk's pending display text.
            if prev_line_fresh {
                drop_trailing_blank_line(&mut display_text);
            }
            sink.emit(RuntimeEvent::SessionCwdChanged(SessionCwdChangedEvent {
                session_id: session_id.to_string(),
                cwd: prompt.cwd.clone(),
            }));

            if was_booting {
                sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
                    session_id: session_id.to_string(),
                    cwd: prompt.cwd.clone(),
                }));
                eprintln!("[session] {} ready (cwd: {})", session_id, prompt.cwd);
            }

            if let Some(exec_id) = pending_exec {
                let status = if was_interrupting {
                    "interrupted"
                } else if prompt.exit_code == 0 {
                    "success"
                } else {
                    "failure"
                };

                sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
                    execution_id: exec_id,
                    session_id: session_id.to_string(),
                    exit_code: prompt.exit_code,
                    finished_at: chrono::Utc::now().to_rfc3339(),
                    status: status.to_string(),
                }));
            }

            sink.emit(RuntimeEvent::SessionExecStateChanged(
                SessionExecStateChangedEvent {
                    session_id: session_id.to_string(),
                    exec_state: SessionExecState::Ready.to_string(),
                    changed_at: chrono::Utc::now().to_rfc3339(),
                },
            ));
        }

        display_text.push_str(&tail_display);

        if !display_text.is_empty() {
            sink.emit(RuntimeEvent::TerminalLine(TerminalLineEvent {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.to_string(),
                execution_id: current_exec_id,
                kind: "stdout".to_string(),
                text: display_text,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }));
        }
    }
}

struct ParsedPrompt {
    cwd: String,
    exit_code: i32,
}

/// What a terminal would show on the row where `line` ends: escape sequences
/// (colour, cursor visibility, window title) are dropped and a carriage return
/// throws away the text it overwrites. A Windows ConPTY wraps the marker row in
/// such sequences.
fn last_visible_row(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut visible = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1b {
            i += 1;
            match bytes.get(i) {
                Some(b'[') => {
                    i += 1;
                    while i < bytes.len() && (0x20..=0x3f).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
                Some(b']') => {
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                Some(_) => i += 1,
                None => {}
            }
        } else if b == 0x0d {
            visible.clear();
            i += 1;
        } else if b == 0x07 {
            i += 1;
        } else {
            let ch_len = line[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            visible.push_str(&line[i..i + ch_len]);
            i += ch_len;
        }
    }
    visible
}

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

/// Marker|nonce|cwd|exit. The nonce must match this session.
/// A missing exit code is not a prompt, and it is not treated as 0.
/// A literal ~ cwd is not stored.
///
/// The exit code is the last `|` field and the nonce is the second, so the cwd
/// in between may itself contain `|`. `%25`, `%0D` and `%0A` in the cwd are
/// decoded (the shells escape `%`, CR and LF). Text before the last carriage
/// return is a redraw that the terminal overwrote, so it is ignored.
///
/// This does not authenticate the sender: any command running in the session
/// can read the nonce from shell state and print a matching line.
fn parse_prompt_line(line: &str, nonce: &str) -> Option<ParsedPrompt> {
    let row = last_visible_row(line.trim_end());
    let visible = row.trim();
    if !visible.starts_with(PROMPT_MARKER) {
        return None;
    }
    let (head, exit) = visible.rsplit_once('|')?;
    let rest = head.strip_prefix(PROMPT_MARKER)?.strip_prefix('|')?;
    let (line_nonce, raw_cwd) = rest.split_once('|')?;
    if line_nonce != nonce {
        return None;
    }
    let cwd = unescape_cwd(raw_cwd.trim());
    if cwd.is_empty() || cwd == "~" || cwd.starts_with("~/") || cwd.starts_with("~\\") {
        return None;
    }
    let exit_code = exit.trim().parse::<i32>().ok()?;
    Some(ParsedPrompt { cwd, exit_code })
}

/// Undo the shells' cwd escaping: `%25` -> `%`, `%0D` -> CR, `%0A` -> LF.
/// Any other `%` is kept as written.
fn unescape_cwd(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(pos) = rest.find('%') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let decoded = [("%25", '%'), ("%0D", '\r'), ("%0A", '\n')]
            .iter()
            .find(|(code, _)| tail.starts_with(code));
        match decoded {
            Some((code, ch)) => {
                out.push(*ch);
                rest = &tail[code.len()..];
            }
            None => {
                out.push('%');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Bound the unfinished-line buffer. When it exceeds MAX_READ_BUFFER only a
/// short tail is kept, and a visible notice line is returned so the cut is
/// not silent.
fn truncate_read_buffer(buffer: &mut String) -> Option<String> {
    if buffer.len() <= MAX_READ_BUFFER {
        return None;
    }
    let mut cut = buffer.len() - READ_BUFFER_TAIL;
    while !buffer.is_char_boundary(cut) {
        cut += 1;
    }
    buffer.drain(..cut);
    Some(format!(
        "[commandui: line too long, {cut} bytes of output dropped]\n"
    ))
}

/// Remove the last complete line of `text` when it is whitespace-only
/// (`out\n\n` becomes `out\n`). Text that does not end in a line break is
/// left alone.
fn drop_trailing_blank_line(text: &mut String) {
    let Some(body) = text.strip_suffix('\n') else {
        return;
    };
    let body = body.strip_suffix('\r').unwrap_or(body);
    let line_start = body.rfind('\n').map(|i| i + 1).unwrap_or(0);
    if body[line_start..].trim().is_empty() {
        text.truncate(line_start);
    }
}

/// Could this unterminated tail be (the start of) a prompt-marker line? Such a
/// tail is held back so a marker is never shown, even when it arrives split.
fn tail_could_be_marker(tail: &str) -> bool {
    let row = last_visible_row(tail);
    let visible = row.trim_start();
    !visible.is_empty()
        && (PROMPT_MARKER.starts_with(visible) || visible.starts_with(PROMPT_MARKER))
}

/// Return the part of the unterminated tail not yet shown and mark it shown,
/// or nothing while the tail might still turn out to be a marker line.
fn take_displayable_tail(record: &mut crate::session::SessionRecord) -> String {
    if record.emitted_tail > record.read_buffer.len() {
        record.emitted_tail = record.read_buffer.len();
    }
    if tail_could_be_marker(&record.read_buffer) {
        return String::new();
    }
    let out = record.read_buffer[record.emitted_tail..].to_string();
    record.emitted_tail = record.read_buffer.len();
    out
}

fn drain_complete_lines(buffer: &mut String) -> String {
    match buffer.rfind('\n') {
        Some(idx) => buffer.drain(..=idx).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CollectingSink;

    /// Test the reader-loop chunk processor directly — no PTY, no Tauri.
    /// This proves the state machine works in isolation.
    #[test]
    fn test_process_reader_chunk_plain_text() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        SessionService::process_reader_chunk(
            &sink_dyn,
            &sessions,
            "test-session",
            "hello world\n",
        );

        assert_eq!(sink.len(), 1);
        let events = sink.events();
        match &events[0] {
            RuntimeEvent::TerminalLine(e) => {
                assert_eq!(e.session_id, "test-session");
                assert_eq!(e.text, "hello world\n");
                assert_eq!(e.kind, "stdout");
            }
            _ => panic!("expected TerminalLine event"),
        }
    }

    #[test]
    fn test_process_reader_chunk_prompt_marker_boot() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        // Insert a booting session
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/old".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: make_dummy_writer(),
                pending_execution_id: None,
                exec_state: SessionExecState::Booting,
                boot_prompt_received: false,
                command_sent_at: None,
            marker_nonce: "test-nonce".to_string(),
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        // Simulate prompt marker arrival
        let marker_line = format!("{}|test-nonce|/home/user|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker_line);

        let events = sink.events();
        // Should emit: CwdChanged, SessionReady, ExecStateChanged
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], RuntimeEvent::SessionCwdChanged(_)));
        assert!(matches!(events[1], RuntimeEvent::SessionReady(_)));
        assert!(matches!(events[2], RuntimeEvent::SessionExecStateChanged(_)));

        // Verify session state was updated
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.cwd, "/home/user");
        assert!(record.boot_prompt_received);
        assert_eq!(record.exec_state, SessionExecState::Ready);
    }

    #[test]
    fn test_process_reader_chunk_execution_completion() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        // Insert a running session with pending execution
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: make_dummy_writer(),
                pending_execution_id: Some("exec-1".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: Some("2026-01-01T00:00:00Z".to_string()),
                marker_nonce: "test-nonce".to_string(),
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        // Simulate prompt marker with exit code 0 (success)
        let marker_line = format!("{}|test-nonce|/tmp|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker_line);

        let events = sink.events();
        // Should emit: CwdChanged, ExecutionFinished, ExecStateChanged
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

        // Verify pending execution was cleared
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert!(record.pending_execution_id.is_none());
        assert_eq!(record.exec_state, SessionExecState::Ready);
    }

    #[test]
    fn marker_leading_blank_line_is_hidden_in_the_same_chunk() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: make_dummy_writer(),
                pending_execution_id: Some("exec-1".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: Some("2026-01-01T00:00:00Z".to_string()),
                marker_nonce: "test-nonce".to_string(),
                read_buffer: String::new(),
                emitted_tail: 0,
                marker_gen: 0,
                child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }
        let chunk = format!("out\n\n{}|test-nonce|/tmp|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &chunk);
        let text = sink
            .events()
            .iter()
            .find_map(|e| match e {
                RuntimeEvent::TerminalLine(t) => Some(t.text.clone()),
                _ => None,
            })
            .expect("terminal line");
        assert_eq!(text, "out\n");
    }

    #[test]
    fn drop_trailing_blank_line_cases() {
        let mut s = "out\n\n".to_string();
        drop_trailing_blank_line(&mut s);
        assert_eq!(s, "out\n");
        let mut s = "out\r\n\r\n".to_string();
        drop_trailing_blank_line(&mut s);
        assert_eq!(s, "out\r\n");
        let mut s = "out\n".to_string();
        drop_trailing_blank_line(&mut s);
        assert_eq!(s, "out\n");
        let mut s = "\n".to_string();
        drop_trailing_blank_line(&mut s);
        assert_eq!(s, "");
    }

    #[test]
    fn test_process_reader_chunk_failure_exit_code() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: make_dummy_writer(),
                pending_execution_id: Some("exec-2".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: Some("2026-01-01T00:00:00Z".to_string()),
                marker_nonce: "test-nonce".to_string(),
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        let marker_line = format!("{}|test-nonce|/tmp|1\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker_line);

        let events = sink.events();
        match &events[1] {
            RuntimeEvent::ExecutionFinished(e) => {
                assert_eq!(e.exit_code, 1);
                assert_eq!(e.status, "failure");
            }
            _ => panic!("expected ExecutionFinished event"),
        }
    }

    #[test]
    fn test_process_reader_chunk_interrupt_completion() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: make_dummy_writer(),
                pending_execution_id: Some("exec-3".to_string()),
                exec_state: SessionExecState::Interrupting,
                boot_prompt_received: true,
                command_sent_at: Some("2026-01-01T00:00:00Z".to_string()),
                marker_nonce: "test-nonce".to_string(),
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        let marker_line = format!("{}|test-nonce|/tmp|130\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker_line);

        let events = sink.events();
        match &events[1] {
            RuntimeEvent::ExecutionFinished(e) => {
                assert_eq!(e.status, "interrupted");
            }
            _ => panic!("expected ExecutionFinished event"),
        }
    }

    #[test]
    fn test_process_reader_chunk_mixed_text_and_marker() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
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
            marker_nonce: "test-nonce".to_string(),
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        // Chunk with output text followed by a prompt marker
        let chunk = format!("some output\n{}|test-nonce|/home|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &chunk);

        let events = sink.events();
        // Marker events emit inline during line processing.
        // Display text accumulates and emits after the loop.
        // So order is: CwdChanged, ExecStateChanged, TerminalLine.
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], RuntimeEvent::SessionCwdChanged(_)));
        assert!(matches!(events[1], RuntimeEvent::SessionExecStateChanged(_)));
        match &events[2] {
            RuntimeEvent::TerminalLine(e) => {
                assert_eq!(e.text, "some output\n");
            }
            _ => panic!("expected TerminalLine event last"),
        }
    }

    #[test]
    fn test_partial_prompt_is_carried_until_complete() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(&sessions, "s1", SessionExecState::Booting, false, None);

        let marker = format!("{}|test-nonce|/home/user|0\n", PROMPT_MARKER);
        let (head, tail) = marker.split_at(PROMPT_MARKER.len() + 2);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", head);

        assert_eq!(sink.len(), 0);
        {
            let reg = sessions.lock().unwrap();
            let record = reg.get("s1").unwrap();
            assert!(!record.boot_prompt_received);
            assert_eq!(record.exec_state, SessionExecState::Booting);
            assert!(record.read_buffer.contains(PROMPT_MARKER));
        }

        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", tail);
        let events = sink.events();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], RuntimeEvent::SessionCwdChanged(_)));
        assert!(matches!(events[1], RuntimeEvent::SessionReady(_)));
        assert!(matches!(events[2], RuntimeEvent::SessionExecStateChanged(_)));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert!(record.boot_prompt_received);
        assert_eq!(record.cwd, "/home/user");
        assert!(record.read_buffer.is_empty());
    }

    #[test]
    fn shell_exit_fails_pending_execution_and_marks_session_dead() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );
        SessionService::handle_session_exit(&sink_dyn, &sessions, "s1");
        assert_eq!(finished_statuses(&sink), vec![("failure".to_string(), 1)]);
        {
            let reg = sessions.lock().unwrap();
            let record = reg.get("s1").unwrap();
            assert_eq!(record.status, "exited");
            assert!(record.pending_execution_id.is_none());
        }
        // Idempotent: a second call emits nothing more.
        let n = sink.len();
        SessionService::handle_session_exit(&sink_dyn, &sessions, "s1");
        assert_eq!(sink.len(), n);
    }

    fn shown_text(sink: &CollectingSink) -> String {
        sink.events()
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn unterminated_prompt_is_displayed_without_a_newline() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(&sessions, "s1", SessionExecState::Ready, true, None);

        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "Name: ");
        assert_eq!(shown_text(&sink), "Name: ");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "ls");
        assert_eq!(shown_text(&sink), "Name: ls");
    }

    #[test]
    fn split_marker_is_parsed_and_never_displayed() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(&sessions, "s1", SessionExecState::Booting, false, None);

        let marker = format!("{}|test-nonce|/home/user|0
", PROMPT_MARKER);
        for piece in [&marker[..5], &marker[5..PROMPT_MARKER.len() + 4], &marker[PROMPT_MARKER.len() + 4..]] {
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", piece);
        }
        assert_eq!(shown_text(&sink), "");
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert!(record.boot_prompt_received);
        assert_eq!(record.cwd, "/home/user");
    }

    #[test]
    fn completing_a_displayed_tail_does_not_duplicate_it() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(&sessions, "s1", SessionExecState::Ready, true, None);

        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "hel");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "lo wor");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "ld
next");
        assert_eq!(shown_text(&sink), "hello world
next");
    }

    fn finished_statuses(sink: &CollectingSink) -> Vec<(String, i32)> {
        sink.events()
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::ExecutionFinished(f) => Some((f.status.clone(), f.exit_code)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn marker_glued_to_unterminated_output_still_finishes() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );

        // printf hello (no newline), then the prompt writes its own newline first.
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "hello");
        let marker = format!("\n{}|test-nonce|/tmp|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);
        assert_eq!(finished_statuses(&sink), vec![("success".to_string(), 0)]);
        let reg = sessions.lock().unwrap();
        assert_eq!(reg.get("s1").unwrap().exec_state, SessionExecState::Ready);
        assert!(reg.get("s1").unwrap().pending_execution_id.is_none());
        drop(reg);
        let shown: String = sink
            .events()
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(shown, "hello\n");
    }

    #[test]
    fn carriage_return_redraw_before_marker_still_finishes() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );
        let chunk = format!("progress 50%\r{}|test-nonce|/tmp|3\r\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &chunk);
        assert_eq!(finished_statuses(&sink), vec![("failure".to_string(), 3)]);
    }

    #[test]
    fn cwd_with_pipe_or_escaped_newline_is_parsed() {
        let parsed = parse_prompt_line(
            &format!("{PROMPT_MARKER}|test-nonce|/work/a|b|c|7\n"),
            "test-nonce",
        )
        .unwrap();
        assert_eq!(parsed.cwd, "/work/a|b|c");
        assert_eq!(parsed.exit_code, 7);

        let parsed = parse_prompt_line(
            &format!("{PROMPT_MARKER}|test-nonce|/work/x%0Ay%250A50%25|0\n"),
            "test-nonce",
        )
        .unwrap();
        assert_eq!(parsed.cwd, "/work/x\ny%0A50%");

        assert!(parse_prompt_line(
            &format!("{PROMPT_MARKER}|other-nonce|/work/a|b|0\n"),
            "test-nonce"
        )
        .is_none());

        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );
        let chunk = format!("{PROMPT_MARKER}|test-nonce|/tmp/a|b|0\n");
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &chunk);
        assert_eq!(finished_statuses(&sink).len(), 1);
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().cwd, "/tmp/a|b");
    }

    #[test]
    fn reading_the_shell_prompt_state_is_not_a_secret_but_is_documented() {
        // The nonce appears in the bootstrap text, so a command that echoes it
        // can forge a marker. This pins that the parser accepts such a line, so
        // nobody assumes the nonce is a security boundary.
        let bootstrap = bootstrap_prompt("bash", "test-nonce").unwrap();
        assert!(bootstrap.contains("test-nonce"));
        let forged = format!("{PROMPT_MARKER}|test-nonce|/chosen|0\n");
        assert!(parse_prompt_line(&forged, "test-nonce").is_some());
    }

    #[test]
    fn unterminated_output_is_bounded_with_a_visible_notice() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(&sessions, "s1", SessionExecState::Running, true, Some("e".to_string()));
        let big = "a".repeat(MAX_READ_BUFFER + 10);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &big);
        {
            let reg = sessions.lock().unwrap();
            assert!(reg.get("s1").unwrap().read_buffer.len() <= READ_BUFFER_TAIL);
        }
        assert!(sink.events().iter().any(|e| matches!(
            e,
            RuntimeEvent::TerminalLine(t) if t.text.contains("dropped")
        )));
        // A marker after the oversized line is still recognised.
        let marker = format!("\n{}|test-nonce|/tmp|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);
        assert_eq!(finished_statuses(&sink), vec![("success".to_string(), 0)]);
    }

    #[test]
    fn test_bad_exit_code_does_not_finish_execution() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );

        let missing = format!("{}|test-nonce|/tmp|\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &missing);
        let not_int = format!("{}|test-nonce|/tmp|nope\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &not_int);

        let events = sink.events();
        assert!(events.iter().all(|e| !matches!(
            e,
            RuntimeEvent::ExecutionFinished(_)
                | RuntimeEvent::SessionExecStateChanged(_)
                | RuntimeEvent::SessionReady(_)
        )));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-1"));
        assert_eq!(record.exec_state, SessionExecState::Running);
    }

    #[test]
    fn test_embedded_marker_and_bootstrap_echo_are_ordinary_output() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-9".to_string()),
        );

        let embedded = format!("output {}|test-nonce|/tmp|0\n", PROMPT_MARKER);
        let echoed = bootstrap_prompt("bash", "test-nonce").unwrap();
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &embedded);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &echoed);

        let events = sink.events();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| matches!(e, RuntimeEvent::TerminalLine(_))));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-9"));
        assert_eq!(record.exec_state, SessionExecState::Running);
        assert!(record.boot_prompt_received);
        assert_eq!(record.cwd, "/tmp");
    }

    #[test]
    fn test_marker_without_session_does_not_emit_ready() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let marker = format!("{}|test-nonce|/home/user|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "missing", &marker);

        let events = sink.events();
        assert!(events.iter().all(|e| !matches!(
            e,
            RuntimeEvent::SessionReady(_) | RuntimeEvent::SessionExecStateChanged(_)
        )));
    }

    #[test]
    fn public_marker_line_does_not_finish_execution() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );

        let public_shape = format!("{}|/tmp|0\n", PROMPT_MARKER);
        let wrong_nonce = format!("{}|not-the-nonce|/tmp|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &public_shape);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &wrong_nonce);

        assert!(sink.events().iter().all(|e| !matches!(
            e,
            RuntimeEvent::ExecutionFinished(_) | RuntimeEvent::SessionExecStateChanged(_)
        )));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-1"));
        assert_eq!(record.exec_state, SessionExecState::Running);
        assert_eq!(record.cwd, "/tmp");
    }

    #[test]
    fn tilde_cwd_is_not_stored_and_does_not_finish() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );

        for cwd in ["~", "~/work", "~\\work"] {
            let line = format!("{}|test-nonce|{cwd}|0\n", PROMPT_MARKER);
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &line);
        }

        assert!(sink.events().iter().all(|e| !matches!(e, RuntimeEvent::ExecutionFinished(_))));
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.cwd, "/tmp");
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-1"));
        assert_eq!(record.exec_state, SessionExecState::Running);
    }

    #[test]
    fn matching_nonce_finishes_with_real_exit_and_cwd() {
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-1".to_string()),
        );

        let line = format!("{}|test-nonce|/var/work|7\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &line);

        let finished = sink.events().into_iter().find_map(|e| match e {
            RuntimeEvent::ExecutionFinished(fin) => Some(fin),
            _ => None,
        });
        let finished = finished.expect("execution should finish");
        assert_eq!(finished.exit_code, 7);
        assert_eq!(finished.status, "failure");
        assert_eq!(finished.execution_id, "exec-1");
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.cwd, "/var/work");
        assert!(record.pending_execution_id.is_none());
        assert_eq!(record.exec_state, SessionExecState::Ready);
    }

    #[test]
    fn prompt_clear_does_not_wipe_a_newer_pending_id() {
        let inner = Arc::new(CollectingSink::new());
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(
            &sessions,
            "s1",
            SessionExecState::Running,
            true,
            Some("exec-old".to_string()),
        );
        let sink = ReenterOnFinish {
            inner: inner.clone(),
            sessions: sessions.clone(),
        };
        let sink_dyn: Arc<dyn RuntimeEventSink> = Arc::new(sink);
        let line = format!("{}|test-nonce|/var/work|0\n", PROMPT_MARKER);
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &line);

        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.pending_execution_id.as_deref(), Some("exec-new"));
        assert_eq!(record.exec_state, SessionExecState::Running);
    }

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
            read_buffer: String::new(),
            emitted_tail: 0,
            marker_gen: 0,
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

    #[test]
    fn marker_row_wrapped_in_escape_sequences_is_still_a_marker() {
        // ConPTY wraps the row in colour, cursor-visibility and title escapes.
        let line = "\x1b[?25l\x1b[93m\x1b]0;title\x07__COMMANDUI_PROMPT__|test-nonce|C:\\work|0\x1b[?25h\r\n";
        let parsed = parse_prompt_line(line, "test-nonce").expect("marker parses");
        assert_eq!(parsed.cwd, "C:\\work");
        assert_eq!(parsed.exit_code, 0);
        // Text before the marker on the same row is still not a marker.
        let embedded = "output __COMMANDUI_PROMPT__|test-nonce|C:\\work|0\r\n";
        assert!(parse_prompt_line(embedded, "test-nonce").is_none());
        // A held-back tail is recognised through the same escapes.
        assert!(tail_could_be_marker("\x1b[?25l__COMMANDUI_PRO"));
        assert!(!tail_could_be_marker("\x1b[?25lC:\\work>"));
    }

    #[test]
    fn conpty_stream_without_line_breaks_still_finishes_the_command() {
        // The shape a ConPTY produced live: the output of `echo probe-ok` and
        // the marker row are separated by cursor positioning, not CR LF.
        let sink = Arc::new(CollectingSink::new());
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        insert_session(&sessions, "s1", SessionExecState::Running, true, Some("e1".to_string()));
        let mut rows = crate::pty::RowNormalizer::default();
        let raw = "\x1b[?25h\x1b[mprobe-ok\x1b[?25l\x1b[15;1H__COMMANDUI_PROMPT__|test-nonce|C:\\work|0\x1b[16;1HC:\\work>";
        // Delivered in two reads cut in the middle of an escape.
        let cut = raw.find("\x1b[15").unwrap() + 3;
        for part in [&raw[..cut], &raw[cut..]] {
            let text = rows.push(part);
            SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &text);
        }
        let events = sink.events();
        assert!(events.iter().any(|e| matches!(e, RuntimeEvent::ExecutionFinished(f) if f.execution_id == "e1" && f.exit_code == 0)));
        let shown: String = events
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(l) => Some(l.text.clone()),
                _ => None,
            })
            .collect();
        assert!(shown.contains("probe-ok"), "{shown:?}");
        assert!(!shown.contains("__COMMANDUI_PROMPT__"), "{shown:?}");
        assert_eq!(sessions.lock().unwrap().get("s1").unwrap().cwd, "C:\\work");
    }
}
