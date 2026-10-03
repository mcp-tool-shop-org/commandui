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
    bootstrap_prompt, clone_reader, default_shell, new_marker_nonce, spawn_reader, spawn_shell,
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

        let (pair, writer) = spawn_shell(&shell, Some(&cwd))?;
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
        spawn_reader(reader, move |text| {
            Self::process_reader_chunk(
                &sink,
                &state_sessions,
                &session_id_for_reader,
                &text,
            );
        });

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
        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
        registry
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;
        Ok(())
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

        // Carry a trailing partial line on the session. Chunks are not lines.
        let (current_exec_id, complete, nonce) = match sessions.lock() {
            Ok(mut reg) => {
                if let Some(record) = reg.get_mut(session_id) {
                    record.read_buffer.push_str(text);
                    let exec_id = record.pending_execution_id.clone();
                    let nonce = record.marker_nonce.clone();
                    let mut complete = drain_complete_lines(&mut record.read_buffer);
                    if let Some(notice) = truncate_read_buffer(&mut record.read_buffer) {
                        complete.push_str(&notice);
                    }
                    (exec_id, complete, Some(nonce))
                } else {
                    let mut scratch = text.to_string();
                    let complete = drain_complete_lines(&mut scratch);
                    (None, complete, None)
                }
            }
            Err(_) => {
                let mut scratch = text.to_string();
                let complete = drain_complete_lines(&mut scratch);
                (None, complete, None)
            }
        };

        for line in complete.split_inclusive('\n') {
            let Some(nonce) = nonce.as_deref() else {
                display_text.push_str(line);
                continue;
            };
            let Some(prompt) = parse_prompt_line(line, nonce) else {
                display_text.push_str(line);
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
                display_text.push_str(line);
                continue;
            };

            // The marker is written after a newline of its own, which shows
            // up as a blank line right before it. Hide that one line.
            drop_trailing_blank_line(&mut display_text);

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

fn persist_bootstrap(
    sessions: &Arc<Mutex<SessionRegistry>>,
    session_id: &str,
    writer: &PtyHandle,
    prompt_cmd: &str,
    shell: &str,
) -> Result<(), String> {
    if let Err(e) = write_raw(writer, prompt_cmd) {
        if let Ok(mut registry) = sessions.lock() {
            registry.remove(session_id);
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
    let trimmed = line.trim();
    let visible = trimmed.rsplit('\r').next().unwrap_or(trimmed).trim_start();
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

/// Remove one whitespace-only last line from `text`.
fn drop_trailing_blank_line(text: &mut String) {
    let body_end = text.trim_end_matches(['\r', '\n']).len();
    if body_end == text.len() {
        return;
    }
    let line_start = text[..body_end].rfind('\n').map(|i| i + 1).unwrap_or(0);
    if text[line_start..body_end].trim().is_empty() {
        text.truncate(line_start);
    }
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
                shell: Some("bash-not-installed".to_string()),
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
