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

/// Characters that must never reach the PTY inside a command line: every
/// control character (C0, DEL, C1 including U+0085), the Unicode line and
/// paragraph separators PowerShell treats as line ends, bidi controls that
/// reorder displayed text, and invisible format characters that hide text.
pub(crate) fn is_forbidden_command_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{2028}' | '\u{2029}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
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
        // A newline or other control byte would be sent as keystrokes: extra
        // prompt cycles, end-of-input, escape sequences. Only a single plain
        // line is run.
        if request.command.chars().any(is_forbidden_command_char) {
            return Err(
                "command contains a newline or control character; run one single-line command at a time"
                    .to_string(),
            );
        }

        let now = chrono::Utc::now().to_rfc3339();

        // Mark Running and stash the pending id before the write, then drop
        // the registry lock. The PTY write blocks; it must not hold that lock.
        let (writer, line) = {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get_mut(&request.session_id)
                .ok_or_else(|| format!("Session not found: {}", request.session_id))?;

            if record.status == "exited" {
                return Err("The shell in this session has exited; open a new session".to_string());
            }

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
            // Roll back only while this execution still owns the record. A
            // partial write can let the reader finish this id before the flush
            // fails, and a newer execute may already hold the session.
            enum Rollback {
                Done,
                NotOurs,
                Missing,
            }
            let outcome = {
                let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
                match registry.get_mut(&request.session_id) {
                    Some(record)
                        if record.pending_execution_id.as_deref()
                            == Some(request.execution_id.as_str())
                            && record.exec_state == SessionExecState::Running =>
                    {
                        record.exec_state = SessionExecState::Ready;
                        record.pending_execution_id = None;
                        record.command_sent_at = None;
                        Rollback::Done
                    }
                    Some(_) => Rollback::NotOurs,
                    None => Rollback::Missing,
                }
            };
            match outcome {
                Rollback::Done => {
                    self.emit_execution_finished(
                        &request.session_id,
                        &request.execution_id,
                        "failure",
                        1,
                    );
                }
                Rollback::NotOurs => {}
                Rollback::Missing => {
                    self.emit_execution_finished(
                        &request.session_id,
                        &request.execution_id,
                        "failure",
                        1,
                    );
                    return Err(format!("Session not found: {}", request.session_id));
                }
            }
            return Err(write_err);
        }

        let still_running = {
            let registry = self.sessions.lock().map_err(|e| e.to_string())?;
            match registry.get(&request.session_id) {
                Some(record) => {
                    let running = record.exec_state == SessionExecState::Running;
                    // Emitted under the registry lock so the reader cannot
                    // finish this command and emit Ready before Running.
                    if running {
                        self.emit_exec_state(&request.session_id, &SessionExecState::Running);
                    }
                    Some(running)
                }
                None => None,
            }
        };
        let Some(still_running) = still_running else {
            self.emit_execution_finished(&request.session_id, &request.execution_id, "failure", 1);
            return Err(format!("Session not found: {}", request.session_id));
        };

        if still_running {
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
        let (writer, seen_pending, seen_state, seen_gen, probe) = {
            let registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get(session_id)
                .ok_or_else(|| format!("Session not found: {session_id}"))?;

            if record.status == "exited" {
                return Err("The shell in this session has exited; open a new session".to_string());
            }

            (
                record.writer.clone(),
                record.pending_execution_id.clone(),
                record.exec_state.clone(),
                record.marker_gen,
                resync_input(&record.shell, &record.marker_nonce),
            )
        };

        // Write the probe first. If it fails nothing was sent, so the session
        // and its in-flight command are left exactly as they were.
        write_raw(&writer, &probe)?;

        // Only move to Booting if the reader has not already answered the
        // probe (or finished the command) while the lock was dropped.
        let transition = {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
            let record = registry
                .get_mut(session_id)
                .ok_or_else(|| format!("Session not found: {session_id}"))?;
            if record.marker_gen == seen_gen
                && record.pending_execution_id == seen_pending
                && record.exec_state == seen_state
            {
                let pending = record.pending_execution_id.take();
                record.exec_state = SessionExecState::Booting;
                record.command_sent_at = None;
                Some(pending)
            } else {
                None
            }
        };

        if let Some(pending) = transition {
            if let Some(exec_id) = pending {
                self.emit_execution_finished(session_id, &exec_id, "interrupted", 130);
            }
            self.emit_exec_state(session_id, &SessionExecState::Booting);
        }
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
    fn execute_rejects_newlines_and_control_bytes() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let svc = TerminalService::new(sessions, sink.clone() as Arc<dyn RuntimeEventSink>);
        for bad in [
            "ls\nrm x",
            "ls\r",
            "a\x04",
            "a\x1b[A",
            "a\x7f",
            "a\x00",
            "a\x03",
            // TAB is rejected on purpose: it would trigger shell completion.
            "a\tb",
            // C1 NEL and the Unicode line/paragraph separators.
            "a\u{85}b",
            "a\u{2028}b",
            "a\u{2029}b",
            // Bidi controls.
            "a\u{202A}b",
            "a\u{202B}b",
            "a\u{202C}b",
            "a\u{202D}b",
            "a\u{202E}b",
            "a\u{2066}b",
            "a\u{2067}b",
            "a\u{2068}b",
            "a\u{2069}b",
            // Invisible format characters.
            "a\u{200B}b",
            "a\u{200C}b",
            "a\u{200D}b",
            "a\u{200E}b",
            "a\u{200F}b",
            "a\u{2060}b",
            "a\u{2061}b",
            "a\u{2062}b",
            "a\u{2063}b",
            "a\u{2064}b",
            "a\u{FEFF}b",
        ] {
            let err = svc
                .execute(ExecuteRequest {
                    execution_id: "e1".to_string(),
                    session_id: "s1".to_string(),
                    command: bad.to_string(),
                    source: "user".to_string(),
                    linked_plan_id: None,
                })
                .unwrap_err();
            assert!(err.contains("control character"), "{err}");
        }
        assert_eq!(sink.len(), 0);
    }

    fn capture_writer(buf: &Arc<Mutex<Vec<u8>>>, sessions: &Arc<Mutex<SessionRegistry>>) -> PtyHandle {
        Arc::new(Mutex::new(Box::new(CaptureWrite {
            buf: buf.clone(),
            sessions: sessions.clone(),
        }) as Box<dyn std::io::Write + Send>))
    }

    #[test]
    fn execute_accepts_plain_single_line_with_non_ascii_text() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
        insert_ready(&sessions, "s1", capture_writer(&captured, &sessions));
        let svc = TerminalService::new(sessions, sink as Arc<dyn RuntimeEventSink>);
        let mut req = exec_request("e1");
        req.command = "echo h\u{e9}llo \u{65e5}\u{672c}".to_string();
        svc.execute(req).expect("plain command must pass the guard");
        let bytes = captured.lock().unwrap().clone();
        assert!(String::from_utf8(bytes).unwrap().ends_with("echo h\u{e9}llo \u{65e5}\u{672c}\n"));
    }

    #[test]
    fn execute_clears_pending_input_before_the_command_per_shell() {
        for (shell, clear) in [
            ("bash", "\x05\x15"),
            ("zsh", "\x05\x15"),
            ("pwsh.exe", "\x1b"),
            ("powershell.exe", "\x1b"),
        ] {
            let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
            let sink = Arc::new(CollectingSink::new());
            let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
            insert_ready(&sessions, "s1", capture_writer(&captured, &sessions));
            sessions.lock().unwrap().get_mut("s1").unwrap().shell = shell.to_string();
            let svc = TerminalService::new(sessions, sink as Arc<dyn RuntimeEventSink>);
            svc.execute(exec_request("e1")).unwrap();
            let bytes = captured.lock().unwrap().clone();
            assert_eq!(String::from_utf8(bytes).unwrap(), format!("{clear}ls\n"), "{shell}");
        }
    }

    #[test]
    fn execute_and_resync_refuse_an_exited_session_without_side_effects() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
        insert_ready(&sessions, "s1", capture_writer(&captured, &sessions));
        sessions.lock().unwrap().get_mut("s1").unwrap().status = "exited".to_string();
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);

        let err = svc.execute(exec_request("e1")).unwrap_err();
        assert!(err.contains("has exited"), "{err}");
        let err = svc.resync("s1").unwrap_err();
        assert!(err.contains("has exited"), "{err}");

        assert_eq!(sink.len(), 0);
        assert!(captured.lock().unwrap().is_empty());
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.exec_state, SessionExecState::Ready);
        assert!(record.pending_execution_id.is_none());
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
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
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
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
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
    fn test_execute_write_failure_after_reader_finished_emits_no_second_finish() {
        struct FinishThenFail {
            sessions: Arc<Mutex<SessionRegistry>>,
            sink: Arc<CollectingSink>,
        }
        impl std::io::Write for FinishThenFail {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                // The reader finished e1 and a newer execute (e2) took over.
                let mut reg = self.sessions.lock().unwrap();
                let record = reg.get_mut("s1").unwrap();
                record.pending_execution_id = Some("e2".to_string());
                record.exec_state = SessionExecState::Running;
                self.sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
                    execution_id: "e1".to_string(),
                    session_id: "s1".to_string(),
                    exit_code: 0,
                    finished_at: "t".to_string(),
                    status: "success".to_string(),
                }));
                Err(std::io::Error::other("flush failed"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        let writer = Arc::new(Mutex::new(Box::new(FinishThenFail {
            sessions: sessions.clone(),
            sink: sink.clone(),
        }) as Box<dyn std::io::Write + Send>));
        insert_ready(&sessions, "s1", writer);
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        assert!(svc.execute(exec_request("e1")).is_err());
        let reg = sessions.lock().unwrap();
        let record = reg.get("s1").unwrap();
        assert_eq!(record.pending_execution_id.as_deref(), Some("e2"));
        assert_eq!(record.exec_state, SessionExecState::Running);
        let finishes = sink
            .events()
            .iter()
            .filter(|e| matches!(e, RuntimeEvent::ExecutionFinished(_)))
            .count();
        assert_eq!(finishes, 1);
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
                emitted_tail: 0,
                marker_gen: 0,
                child: None,
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
        assert!(text.starts_with("\x1bdir\ncall set __cui_ec=%^ERRORLEVEL% & "));
        assert!(text.contains("!__cui_ec!"));
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
                emitted_tail: 0,
                marker_gen: 0,
                child: None,
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
                emitted_tail: 0,
                marker_gen: 0,
                child: None,
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
    fn resync_write_failure_leaves_the_session_untouched() {
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
                emitted_tail: 0,
                marker_gen: 0,
                child: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                last_active_at: "2026-01-01T00:00:00Z".to_string(),
            });
        }
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        assert!(svc.resync("s1").is_err());
        {
            let reg = sessions.lock().unwrap();
            let record = reg.get("s1").unwrap();
            assert_eq!(record.pending_execution_id.as_deref(), Some("e9"));
            assert_eq!(record.exec_state, SessionExecState::Running);
        }
        assert!(sink.events().iter().all(|e| !matches!(
            e,
            RuntimeEvent::ExecutionFinished(_) | RuntimeEvent::SessionExecStateChanged(_)
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

    #[test]
    fn interrupt_and_resync_missing_session() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = TerminalService::new(sessions, sink);
        let interrupted = svc.interrupt("missing").unwrap_err();
        assert!(interrupted.contains("Session not found"), "{interrupted}");
        let resynced = svc.resync("missing").unwrap_err();
        assert!(resynced.contains("Session not found"), "{resynced}");
    }

    #[test]
    fn resize_missing_session_and_open_pty() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let svc = TerminalService::new(sessions.clone(), sink);
        let missing = svc.resize("missing", 80, 24).unwrap_err();
        assert!(missing.contains("Session not found"), "{missing}");
        insert_ready(&sessions, "s1", make_dummy_writer());
        svc.resize("s1", 100, 40).unwrap();
    }

    #[test]
    fn execute_rejects_booting_desynced_and_interrupting() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        insert_ready(&sessions, "s1", make_dummy_writer());
        let svc = TerminalService::new(sessions.clone(), sink as Arc<dyn RuntimeEventSink>);

        sessions.lock().unwrap().get_mut("s1").unwrap().exec_state = SessionExecState::Booting;
        let booting = svc
            .execute(exec_request("e-boot"))
            .unwrap_err();
        assert!(booting.contains("still booting"), "{booting}");

        sessions.lock().unwrap().get_mut("s1").unwrap().exec_state = SessionExecState::Desynced;
        let desynced = svc.execute(exec_request("e-desync")).unwrap_err();
        assert!(desynced.contains("desynced"), "{desynced}");

        sessions.lock().unwrap().get_mut("s1").unwrap().exec_state = SessionExecState::Interrupting;
        let interrupting = svc.execute(exec_request("e-int")).unwrap_err();
        assert!(interrupting.contains("already running"), "{interrupting}");
    }

    #[test]
    fn registry_lock_poison_is_an_error() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let to_poison = sessions.clone();
        let joined = std::thread::spawn(move || {
            let _guard = to_poison.lock().unwrap();
            panic!("poison session registry");
        })
        .join();
        assert!(joined.is_err());
        let svc = TerminalService::new(sessions, sink);
        assert!(svc.interrupt("s1").is_err());
        assert!(svc.resync("s1").is_err());
        assert!(svc.write("s1", "x").is_err());
        assert!(svc.resize("s1", 80, 24).is_err());
        assert!(svc.execute(exec_request("e1")).is_err());
    }

    #[test]
    fn execute_lock_errors_after_the_write() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        insert_ready(
            &sessions,
            "s1",
            Arc::new(Mutex::new(Box::new(PoisonRegistry {
                sessions: sessions.clone(),
                fail_write: false,
            }) as Box<dyn std::io::Write + Send>)),
        );
        let svc = TerminalService::new(sessions.clone(), sink.clone() as Arc<dyn RuntimeEventSink>);
        assert!(svc.execute(exec_request("e-ok-write")).is_err());

        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink = Arc::new(CollectingSink::new());
        insert_ready(
            &sessions,
            "s1",
            Arc::new(Mutex::new(Box::new(PoisonRegistry {
                sessions: sessions.clone(),
                fail_write: true,
            }) as Box<dyn std::io::Write + Send>)),
        );
        let svc = TerminalService::new(sessions, sink as Arc<dyn RuntimeEventSink>);
        assert!(svc.execute(exec_request("e-bad-write")).is_err());
    }

    #[test]
    fn interrupt_write_failure_when_session_is_already_gone() {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(CollectingSink::new());
        let writer = Arc::new(Mutex::new(Box::new(RemoveThenFail {
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
                writer,
                pending_execution_id: Some("e0".to_string()),
                exec_state: SessionExecState::Running,
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
        let svc = TerminalService::new(sessions.clone(), sink);
        let err = svc.interrupt("s1").unwrap_err();
        assert!(err.contains("Write error"), "{err}");
        assert!(sessions.lock().unwrap().get("s1").is_none());
    }

    fn exec_request(execution_id: &str) -> ExecuteRequest {
        ExecuteRequest {
            execution_id: execution_id.to_string(),
            session_id: "s1".to_string(),
            command: "ls".to_string(),
            source: "user".to_string(),
            linked_plan_id: None,
        }
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
            emitted_tail: 0,
            marker_gen: 0,
            child: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_active_at: "2026-01-01T00:00:00Z".to_string(),
        });
    }

    fn failing_writer() -> crate::pty::PtyHandle {
        Arc::new(Mutex::new(Box::new(FailWrite) as Box<dyn std::io::Write + Send>))
    }

    struct RemoveThenFail {
        sessions: Arc<Mutex<SessionRegistry>>,
    }

    impl std::io::Write for RemoveThenFail {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            let _ = self.sessions.lock().unwrap().remove("s1");
            Err(std::io::Error::other("write failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct PoisonRegistry {
        sessions: Arc<Mutex<SessionRegistry>>,
        fail_write: bool,
    }

    impl std::io::Write for PoisonRegistry {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.fail_write {
                poison_registry(&self.sessions);
                return Err(std::io::Error::other("write failed"));
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if !self.fail_write {
                poison_registry(&self.sessions);
            }
            Ok(())
        }
    }

    fn poison_registry(sessions: &Arc<Mutex<SessionRegistry>>) {
        let sessions = sessions.clone();
        let joined = std::thread::spawn(move || {
            let _guard = sessions.lock().unwrap();
            panic!("poison session registry");
        })
        .join();
        assert!(joined.is_err());
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
