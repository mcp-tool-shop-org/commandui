//! Terminal command service.
//!
//! Owns command execution, interrupt, resync, write, and resize.
//! Manages exec-state gating and event emission.
//!
//! No adapter-specific types (Tauri, Ratatui) belong here.

use crate::events::{
    ExecutionStartedEvent, ExecutionSummary, RuntimeEvent, RuntimeEventSink,
    SessionExecStateChangedEvent,
};
use crate::pty::{write_command, write_raw};
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

        // Double-submit guard: check exec state before proceeding
        {
            let registry = self.sessions.lock().map_err(|e| e.to_string())?;

            let record = registry
                .get(&request.session_id)
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
                SessionExecState::Ready => {} // proceed
            }

            // Write command while we hold the lock
            write_command(&record.writer, &request.command)?;
        }

        let now = chrono::Utc::now().to_rfc3339();
        let summary = ExecutionSummary {
            id: request.execution_id.clone(),
            session_id: request.session_id.clone(),
            command: request.command,
            source: request.source,
            linked_plan_id: request.linked_plan_id,
            status: "running".to_string(),
            started_at: now.clone(),
            finished_at: None,
            exit_code: None,
        };

        self.event_sink.emit(RuntimeEvent::ExecutionStarted(
            ExecutionStartedEvent {
                execution: summary.clone(),
            },
        ));

        // Update state: Running + pending execution
        {
            let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

            if let Some(record) = registry.get_mut(&request.session_id) {
                record.exec_state = SessionExecState::Running;
                record.command_sent_at = Some(now);
                record.pending_execution_id = Some(request.execution_id);
            }
        }

        self.emit_exec_state(&request.session_id, &SessionExecState::Running);

        Ok(summary)
    }

    pub fn interrupt(&self, session_id: &str) -> Result<(), String> {
        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

        let record = registry
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;

        if record.exec_state != SessionExecState::Running {
            return Err("No command is currently running".to_string());
        }

        // Send Ctrl+C (ETX byte)
        write_raw(&record.writer, "\x03")?;

        record.exec_state = SessionExecState::Interrupting;

        drop(registry);

        self.emit_exec_state(session_id, &SessionExecState::Interrupting);
        eprintln!("[terminal] interrupt sent to session {}", session_id);

        Ok(())
    }

    pub fn resync(&self, session_id: &str) -> Result<(), String> {
        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;

        let record = registry
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;

        // Send newline to provoke a new prompt
        write_raw(&record.writer, "\n")?;

        // Reset state — wait for prompt marker to transition back to Ready
        record.exec_state = SessionExecState::Booting;
        record.pending_execution_id = None;
        record.command_sent_at = None;

        drop(registry);

        self.emit_exec_state(session_id, &SessionExecState::Booting);
        eprintln!("[terminal] resync initiated for session {}", session_id);

        Ok(())
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let registry = self.sessions.lock().map_err(|e| e.to_string())?;

        let record = registry
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;

        write_raw(&record.writer, data)?;

        Ok(())
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let mut registry = self.sessions.lock().map_err(|e| e.to_string())?;
        registry.resize(session_id, cols, rows)
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

    fn make_dummy_writer() -> PtyHandle {
        Arc::new(Mutex::new(Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>))
    }
}
