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
use crate::pty::{bootstrap_prompt, default_shell, spawn_reader_loop, spawn_shell, write_raw, PROMPT_MARKER};
use crate::session::{SessionExecState, SessionRecord, SessionRegistry};
use std::sync::{Arc, Mutex};

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
        let cwd = request
            .cwd
            .unwrap_or_else(|| std::env::current_dir().unwrap().to_string_lossy().to_string());
        let now = chrono::Utc::now().to_rfc3339();

        let (pair, writer) = spawn_shell(&shell, Some(&cwd))?;

        // Start the reader loop with runtime-owned inference
        let session_id_for_reader = id.clone();
        let state_sessions = self.sessions.clone();
        let sink = self.event_sink.clone();

        spawn_reader_loop(&pair, move |text| {
            Self::process_reader_chunk(
                &sink,
                &state_sessions,
                &session_id_for_reader,
                &text,
            );
        });

        // Inject prompt marker
        if let Some(prompt_cmd) = bootstrap_prompt(&shell) {
            write_raw(&writer, &prompt_cmd).ok();
        }

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
            created_at: now.clone(),
            last_active_at: now.clone(),
        };

        let summary = SessionSummary {
            id: id.clone(),
            label,
            cwd,
            shell,
            status: "active".to_string(),
            created_at: now.clone(),
            last_active_at: now,
        };

        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
        registry.insert(record);

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

        // Read current execution_id for attribution
        let (current_exec_id, _current_exec_state) = sessions
            .lock()
            .ok()
            .and_then(|reg| {
                let record = reg.get(session_id)?;
                Some((
                    record.pending_execution_id.clone(),
                    record.exec_state.clone(),
                ))
            })
            .unwrap_or((None, SessionExecState::Booting));

        for line in text.split_inclusive('\n') {
            if line.contains(PROMPT_MARKER) {
                // Parse marker: __COMMANDUI_PROMPT__|cwd|exitcode
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 2 {
                    let cwd = parts[1].to_string();
                    let exit_code = if parts.len() >= 3 {
                        parts[2].trim().parse::<i32>().unwrap_or(0)
                    } else {
                        0
                    };

                    // Update session cwd + check boot status
                    let (was_booting, pending_exec, was_interrupting) = {
                        if let Ok(mut reg) = sessions.lock() {
                            if let Some(record) = reg.get_mut(session_id) {
                                record.cwd = cwd.clone();
                                let was_boot = !record.boot_prompt_received;
                                let was_int =
                                    record.exec_state == SessionExecState::Interrupting;
                                let pending = record.pending_execution_id.clone();

                                if was_boot {
                                    record.boot_prompt_received = true;
                                }

                                // Transition to Ready
                                record.exec_state = SessionExecState::Ready;
                                record.command_sent_at = None;

                                (was_boot, pending, was_int)
                            } else {
                                (false, None, false)
                            }
                        } else {
                            (false, None, false)
                        }
                    };

                    // Emit cwd changed
                    sink.emit(RuntimeEvent::SessionCwdChanged(SessionCwdChangedEvent {
                        session_id: session_id.to_string(),
                        cwd: cwd.clone(),
                    }));

                    // Boot detection — first prompt marker
                    if was_booting {
                        sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
                            session_id: session_id.to_string(),
                            cwd: cwd.clone(),
                        }));
                        eprintln!("[session] {} ready (cwd: {})", session_id, cwd);
                    }

                    // Execution completion
                    if let Some(exec_id) = pending_exec {
                        let status = if was_interrupting {
                            "interrupted"
                        } else if exit_code == 0 {
                            "success"
                        } else {
                            "failure"
                        };

                        sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
                            execution_id: exec_id,
                            session_id: session_id.to_string(),
                            exit_code,
                            finished_at: chrono::Utc::now().to_rfc3339(),
                            status: status.to_string(),
                        }));

                        if let Ok(mut reg) = sessions.lock() {
                            let _ = reg.set_pending_execution(session_id, None);
                        }
                    }

                    // Emit state change to Ready
                    sink.emit(RuntimeEvent::SessionExecStateChanged(
                        SessionExecStateChangedEvent {
                            session_id: session_id.to_string(),
                            exec_state: SessionExecState::Ready.to_string(),
                            changed_at: chrono::Utc::now().to_rfc3339(),
                        },
                    ));
                }
            } else {
                display_text.push_str(line);
            }
        }

        // Emit display text via sink
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
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        // Simulate prompt marker arrival
        let marker_line = format!("{}|/home/user|0\n", PROMPT_MARKER);
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
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        // Simulate prompt marker with exit code 0 (success)
        let marker_line = format!("{}|/tmp|0\n", PROMPT_MARKER);
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
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        let marker_line = format!("{}|/tmp|1\n", PROMPT_MARKER);
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
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        let marker_line = format!("{}|/tmp|130\n", PROMPT_MARKER);
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
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        // Chunk with output text followed by a prompt marker
        let chunk = format!("some output\n{}|/home|0\n", PROMPT_MARKER);
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

    // --- Test helpers ---

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
