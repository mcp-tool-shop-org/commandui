//! Terminal command service.
//!
//! Owns command execution, interrupt, resync, write, and resize.
//! Manages exec-state gating and event emission.
//!
//! No adapter-specific types (Tauri, Ratatui) belong here.

use crate::events::{
    ExecutionFinishedEvent, ExecutionStartedEvent, ExecutionSummary, RuntimeEvent, RuntimeEventSink,
    SessionExecStateChangedEvent,
};
use crate::pty::{command_line_for_shell, resync_input, write_raw};
use crate::session::{SessionExecState, SessionRegistry};
use std::sync::{Arc, Mutex};

/// Request to execute a command in a session.
pub struct ExecuteRequest {
    pub execution_id: String,
    pub session_id: String,
    pub command: String,
    pub source: String,
    pub linked_plan_id: Option<String>,
}

pub struct TerminalService {
    sessions: Arc<Mutex<SessionRegistry>>,
    event_sink: Arc<dyn RuntimeEventSink>,
}

impl TerminalService {
    pub fn new(
        sessions: Arc<Mutex<SessionRegistry>>,
        event_sink: Arc<dyn RuntimeEventSink>,
    ) -> Self {
        Self {
            sessions,
            event_sink,
        }
    }

    pub fn execute(&self, request: ExecuteRequest) -> Result<ExecutionSummary, String> {
        if request.command.is_empty() {
            return Err("command cannot be empty".to_string());
        }

        let now = chrono::Utc::now().to_rfc3339();

        // Mark Running and stash the pending id before the write, then drop
        // the registry lock. The PTY write blocks; it must not hold that lock.
        let (writer, line) = {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get_mut(&request.session_id)
                .ok_or_else(|| format!("Session not found: {}", request.session_id))?;

            match record.exec_state {
                SessionExecState::Running | SessionExecState::Interrupting => {
                    return Err("A command is already running in this session".to_string());
                }
                SessionExecState::Booting => {
                    return Err("Session is still booting".to_string());
                }
                SessionExecState::Desynced => {
                    return Err("Session is desynced — resync first".to_string());
                }
                SessionExecState::Ready => {}
            }

            record.exec_state = SessionExecState::Running;
            record.pending_execution_id = Some(request.execution_id.clone());
            record.command_sent_at = Some(now.clone());
            let line = command_line_for_shell(&record.shell, &record.marker_nonce, &request.command);
            (record.writer.clone(), line)
        };

        let summary = ExecutionSummary {
            id: request.execution_id.clone(),
            session_id: request.session_id.clone(),
            command: request.command.clone(),
            source: request.source,
            linked_plan_id: request.linked_plan_id,
            status: "running".to_string(),
            started_at: now,
            finished_at: None,
            exit_code: None,
        };

        self.event_sink.emit(RuntimeEvent::ExecutionStarted(
            ExecutionStartedEvent {
                execution: summary.clone(),
            },
        ));

        if let Err(write_err) = write_raw(&writer, &line) {
            let found = {
                let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
                if let Some(record) = registry.get_mut(&request.session_id) {
                    record.exec_state = SessionExecState::Ready;
                    record.pending_execution_id = None;
                    record.command_sent_at = None;
                    true
                } else {
                    false
                }
            };
            self.emit_execution_finished(&request.session_id, &request.execution_id, "failure", 1);
            if !found {
                return Err(format!("Session not found: {}", request.session_id));
            }
            return Err(write_err);
        }

        let still_running = {
            let registry = self.sessions.lock().map_err(|e| e.to_string())?;
            match registry.get(&request.session_id) {
                Some(record) => Some(record.exec_state == SessionExecState::Running),
                None => None,
            }
        };
        let Some(still_running) = still_running else {
            self.emit_execution_finished(&request.session_id, &request.execution_id, "failure", 1);
            return Err(format!("Session not found: {}", request.session_id));
        };

        if still_running {
            self.emit_exec_state(&request.session_id, &SessionExecState::Running);
            Ok(summary)
        } else {
            let mut done = summary;
            done.status = "finished".to_string();
            done.finished_at = Some(chrono::Utc::now().to_rfc3339());
            Ok(done)
        }
    }

    pub fn interrupt(&self, session_id: &str) -> Result<(), String> {
        let writer = {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get_mut(session_id)
                .ok_or_else(|| format!("Session not found: {session_id}"))?;

            if record.exec_state != SessionExecState::Running {
                return Err("No command is currently running".to_string());
            }

            record.exec_state = SessionExecState::Interrupting;
            record.writer.clone()
        };

        // Ctrl+C after the lock is dropped. A blocked PTY write must not stall the reader.
        if let Err(e) = write_raw(&writer, "\x03") {
            let mut registry = self.sessions.lock().map_err(|err| err.to_string())?;
            if let Some(record) = registry.get_mut(session_id) {
                record.exec_state = SessionExecState::Running;
            }
            return Err(e);
        }

        self.emit_exec_state(session_id, &SessionExecState::Interrupting);
        eprintln!("[terminal] interrupt sent to session {}", session_id);

        Ok(())
    }

    pub fn resync(&self, session_id: &str) -> Result<(), String> {
        let (writer, pending, probe) = {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get_mut(session_id)
                .ok_or_else(|| format!("Session not found: {session_id}"))?;

            let pending = record.pending_execution_id.take();
            let probe = resync_input(&record.shell, &record.marker_nonce);
            record.exec_state = SessionExecState::Booting;
            record.command_sent_at = None;
            (record.writer.clone(), pending, probe)
        };

        if let Some(exec_id) = pending {
            self.emit_execution_finished(session_id, &exec_id, "interrupted", 130);
        }

        self.emit_exec_state(session_id, &SessionExecState::Booting);

        write_raw(&writer, &probe)?;
        eprintln!("[terminal] resync initiated for session {}", session_id);

        Ok(())
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let writer = {
            let registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get(session_id)
                .ok_or_else(|| format!("Session not found: {session_id}"))?;

            record.writer.clone()
        };

        write_raw(&writer, data)
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
        registry.resize(session_id, cols, rows)
    }

    fn emit_execution_finished(
        &self,
        session_id: &str,
        execution_id: &str,
        status: &str,
        exit_code: i32,
    ) {
        self.event_sink.emit(RuntimeEvent::ExecutionFinished(
            ExecutionFinishedEvent {
                execution_id: execution_id.to_string(),
                session_id: session_id.to_string(),
                exit_code,
                finished_at: chrono::Utc::now().to_rfc3339(),
                status: status.to_string(),
            },
        ));
    }

    fn emit_exec_state(&self, session_id: &str, exec_state: &SessionExecState) {
        self.event_sink.emit(RuntimeEvent::SessionExecStateChanged(
            SessionExecStateChangedEvent {
                session_id: session_id.to_string(),
                exec_state: exec_state.to_string(),
                changed_at: chrono::Utc::now().to_rfc3339(),
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CollectingSink;
    use crate::pty::PtyHandle;
    use crate::session::SessionRecord;

    #[test]
    fn test_execute_empty_command_rejected() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let svc = TerminalService::new(sessions, sink.clone() as Arc<dyn RuntimeEventSink>);

        let result = svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            command: "".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        });

        assert!(result.is_err());
        assert_eq!(sink.len(), 0); // no events emitted
    }

    #[test]
    fn test_execute_session_not_found() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = TerminalService::new(sessions, sink);

        let result = svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "nonexistent".to_string(),
            command: "ls".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_execute_rejects_during_running() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());

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
                pending_execution_id: Some("e0".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: None,
                marker_nonce: "test-nonce".to_string(),
            read_buffer: String::new(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }

        let svc = TerminalService::new(sessions, sink);
        let result = svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            command: "ls".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already running"));
    }

    #[test]
    fn test_interrupt_not_running_rejected() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());

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

        let svc = TerminalService::new(sessions, sink);
        let result = svc.interrupt("s1");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("No command is currently running"), "unexpected error: {err}");
    }

    #[test]
    fn test_execute_sets_running_before_returning() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        insert_ready(&sessions, "s1", make_dummy_writer());

        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        let result = svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            command: "ls".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        });

        assert!(result.is_ok());
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.exec_state, SessionExecState::Running);
        assert_eq!(record.pending_execution_id.as_deref(), Some("e1"));
        drop(reg);
        assert!(sink.len() >= 1);
    }

    #[test]
    fn test_execute_write_failure_rolls_back_to_ready() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        insert_ready(&sessions, "s1", failing_writer());

        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        let result = svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            command: "ls".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        });

        assert!(result.is_err());
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.exec_state, SessionExecState::Ready);
        assert!(record.pending_execution_id.is_none());
        drop(reg);
        let events = sink.events();
        assert!(matches!(events.first(), Some(RuntimeEvent::ExecutionStarted(_))));
        assert!(matches!(
            events.last(),
            Some(RuntimeEvent::ExecutionFinished(fin))
                if fin.execution_id == "e1" && fin.status == "failure"
        ));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, RuntimeEvent::ExecutionStarted(_)))
                .count(),
            1
        );
    }

    #[test]
    fn test_execute_missing_session_after_write_is_err() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let writer = Arc::new(Mutex::new(
            Box::new(DropSessionOnWrite {
                sessions: sessions.clone(),
                session_id: "s1".to_string(),
            }) as Box<dyn std::io::Write + Send>,
        ));
        insert_ready(&sessions, "s1", writer);

        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        let result = svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            command: "ls".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
        assert!(sessions.lock().unwrap().get("s1").is_none());
        let events = sink.events();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, RuntimeEvent::ExecutionStarted(_)))
                .count(),
            1
        );
        assert!(events.iter().any(|e| matches!(
            e,
            RuntimeEvent::ExecutionFinished(fin) if fin.status == "failure"
        )));
    }

    #[test]
    fn test_execute_finished_during_write_does_not_return_running() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let writer = Arc::new(Mutex::new(Box::new(FinishDuringWrite {
            sessions: sessions.clone(),
            sink: sink.clone(),
        }) as Box<dyn std::io::Write + Send>));
        insert_ready(&sessions, "s1", writer);

        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        let result = svc
            .execute(ExecuteRequest {
                execution_id: "e1".to_string(),
                session_id: "s1".to_string(),
                command: "ls".to_string(),
                source: "user".to_string(),
                linked_plan_id: None,
            })
            .unwrap();
        assert_ne!(result.status, "running");
        let events = sink.events();
        let starts: Vec<_> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, RuntimeEvent::ExecutionStarted(_)))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(starts.len(), 1);
        let finish = events
            .iter()
            .position(|e| matches!(e, RuntimeEvent::ExecutionFinished(_)))
            .unwrap();
        assert!(starts[0] < finish);
        let reg = sessions.lock().unwrap();
        assert_eq!(reg.get("s1").unwrap().exec_state, SessionExecState::Ready);
        assert!(reg.get("s1").unwrap().pending_execution_id.is_none());
    }

    #[test]
    fn test_execute_cmd_line_includes_exit_marker() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(Mutex::new(Box::new(CaptureWrite {
            buf: captured.clone(),
            sessions: sessions.clone(),
        }) as Box<dyn std::io::Write + Send>));
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "cmd.exe".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer,
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
        let svc = TerminalService::new(sessions, sink as Arc<dyn RuntimeEventSink>);
        svc.execute(ExecuteRequest {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            command: "dir".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        })
        .unwrap();
        let bytes = captured.lock().unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("dir & echo "));
        assert!(text.contains("!ERRORLEVEL!"));
        assert!(text.contains("test-nonce"));
        assert!(!text.contains('~'));
    }

    #[test]
    fn pty_writes_do_not_hold_the_registry_lock() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let writer = Arc::new(Mutex::new(Box::new(TryLockWriter {
            sessions: sessions.clone(),
        }) as Box<dyn std::io::Write + Send>));
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: writer.clone(),
                pending_execution_id: Some("e0".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: None,
                marker_nonce: "test-nonce".to_string(),
                read_buffer: String::new(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        svc.interrupt("s1").unwrap();
        assert_eq!(
            sessions.lock().unwrap().get("s1").unwrap().exec_state,
            SessionExecState::Interrupting
        );
        {
            let mut reg = sessions.lock().unwrap();
            let record = reg.get_mut("s1").unwrap();
            record.exec_state = SessionExecState::Running;
            record.pending_execution_id = Some("e0".to_string());
        }
        svc.resync("s1").unwrap();
        {
            let reg = sessions.lock().unwrap();
            let record = reg.get("s1").unwrap();
            assert_eq!(record.exec_state, SessionExecState::Booting);
            assert!(record.pending_execution_id.is_none());
        }
        let finished = sink.events().into_iter().any(|e| {
            matches!(
                e,
                RuntimeEvent::ExecutionFinished(fin)
                    if fin.execution_id == "e0" && fin.status == "interrupted"
            )
        });
        assert!(finished);
        svc.write("s1", "abc").unwrap();
    }

    #[test]
    fn interrupt_write_failure_restores_running() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: failing_writer(),
                pending_execution_id: Some("e0".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: None,
                marker_nonce: "test-nonce".to_string(),
                read_buffer: String::new(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }
        let svc = TerminalService::new(sessions.clone(), sink);
        assert!(svc.interrupt("s1").is_err());
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.exec_state, SessionExecState::Running);
        assert_eq!(record.pending_execution_id.as_deref(), Some("e0"));
    }

    #[test]
    fn resync_without_pending_does_not_emit_finished() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        insert_ready(&sessions, "s1", make_dummy_writer());
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        svc.resync("s1").unwrap();
        assert!(sink.events().iter().all(|e| !matches!(e, RuntimeEvent::ExecutionFinished(_))));
        assert_eq!(
            sessions.lock().unwrap().get("s1").unwrap().exec_state,
            SessionExecState::Booting
        );
    }

    #[test]
    fn resync_write_failure_still_finishes_pending() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        {
            let mut reg = sessions.lock().unwrap();
            reg.insert(SessionRecord {
                id: "s1".to_string(),
                label: "Test".to_string(),
                cwd: "/tmp".to_string(),
                shell: "bash".to_string(),
                status: "active".to_string(),
                pty_pair: make_dummy_pty_pair(),
                writer: failing_writer(),
                pending_execution_id: Some("e9".to_string()),
                exec_state: SessionExecState::Running,
                boot_prompt_received: true,
                command_sent_at: None,
                marker_nonce: "test-nonce".to_string(),
                read_buffer: String::new(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        assert!(svc.resync("s1").is_err());
        assert!(sessions.lock().unwrap().get("s1").unwrap().pending_execution_id.is_none());
        assert!(sink.events().iter().any(|e| matches!(
            e,
            RuntimeEvent::ExecutionFinished(fin)
                if fin.execution_id == "e9" && fin.status == "interrupted"
        )));
    }

    #[test]
    fn write_missing_session_and_write_error() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = TerminalService::new(sessions.clone(), sink);
        assert!(svc.write("missing", "x").unwrap_err().contains("not found"));
        insert_ready(&sessions, "s1", failing_writer());
        assert!(svc.write("s1", "x").is_err());
    }

    // --- Test helpers ---

    fn insert_ready(sessions: &Arc<Mutex<SessionRegistry>>, id: &str, writer: crate::pty::PtyHandle) {
        let mut reg = sessions.lock().unwrap();
        reg.insert(SessionRecord {
            id: id.to_string(),
            label: "Test".to_string(),
            cwd: "/tmp".to_string(),
            shell: "bash".to_string(),
            status: "active".to_string(),
            pty_pair: make_dummy_pty_pair(),
            writer,
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

    fn failing_writer() -> crate::pty::PtyHandle {
        Arc::new(Mutex::new(Box::new(FailWrite) as Box<dyn std::io::Write + Send>))
    }

    struct FailWrite;

    impl std::io::Write for FailWrite {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "write failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct DropSessionOnWrite {
        sessions: Arc<Mutex<SessionRegistry>>,
        session_id: String,
    }

    impl std::io::Write for DropSessionOnWrite {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _ = self.sessions.lock().unwrap().remove(&self.session_id);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
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

    fn make_dummy_writer() -> PtyHandle {
        Arc::new(Mutex::new(Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>))
    }

    struct TryLockWriter {
        sessions: Arc<Mutex<SessionRegistry>>,
    }

    impl std::io::Write for TryLockWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.sessions.try_lock().is_err() {
                return Err(std::io::Error::other("registry mutex held across pty write"));
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct CaptureWrite {
        buf: Arc<Mutex<Vec<u8>>>,
        sessions: Arc<Mutex<SessionRegistry>>,
    }

    impl std::io::Write for CaptureWrite {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.sessions.try_lock().is_err() {
                return Err(std::io::Error::other("registry mutex held across pty write"));
            }
            self.buf.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct FinishDuringWrite {
        sessions: Arc<Mutex<SessionRegistry>>,
        sink: Arc<CollectingSink>,
    }

    impl std::io::Write for FinishDuringWrite {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let nonce = {
                let reg = self.sessions.try_lock().map_err(|_| {
                    std::io::Error::other("registry mutex held across pty write")
                })?;
                reg.get("s1").unwrap().marker_nonce.clone()
            };
            let marker = format!("{}|{nonce}|/tmp|0\n", crate::pty::PROMPT_MARKER);
            let sink_dyn: Arc<dyn RuntimeEventSink> = self.sink.clone();
            crate::services::session_service::SessionService::process_reader_chunk(
                &sink_dyn,
                &self.sessions,
                "s1",
                &marker,
            );
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
