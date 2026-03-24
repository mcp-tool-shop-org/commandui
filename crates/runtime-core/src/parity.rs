//! Golden-path parity tests — drift gate for shared runtime event law.
//!
//! These tests lock the semantic event sequences that both Desktop and Console
//! rely on. If these tests fail, shared law has changed and both shells
//! must be checked.

#[cfg(test)]
mod tests {
    use crate::events::*;
    use crate::pty::PtyHandle;
    use crate::services::session_service::SessionService;
    use crate::session::{SessionExecState, SessionRecord, SessionRegistry};
    use std::sync::{Arc, Mutex};

    fn make_sink() -> Arc<CollectingSink> {
        Arc::new(CollectingSink::new())
    }

    fn make_sessions() -> Arc<Mutex<SessionRegistry>> {
        Arc::new(Mutex::new(SessionRegistry::new()))
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
            .expect("Failed to create dummy PTY")
    }

    fn make_dummy_writer() -> PtyHandle {
        Arc::new(Mutex::new(
            Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>,
        ))
    }

    fn insert_booting_session(sessions: &Arc<Mutex<SessionRegistry>>, id: &str) {
        let mut reg = sessions.lock().unwrap();
        reg.insert(SessionRecord {
            id: id.to_string(),
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

    fn insert_running_session(
        sessions: &Arc<Mutex<SessionRegistry>>,
        id: &str,
        exec_id: &str,
    ) {
        let mut reg = sessions.lock().unwrap();
        reg.insert(SessionRecord {
            id: id.to_string(),
            label: "Test".to_string(),
            cwd: "/home/user".to_string(),
            shell: "bash".to_string(),
            status: "active".to_string(),
            pty_pair: make_dummy_pty_pair(),
            writer: make_dummy_writer(),
            pending_execution_id: Some(exec_id.to_string()),
            exec_state: SessionExecState::Running,
            boot_prompt_received: true,
            command_sent_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_active_at: "2026-01-01T00:00:00Z".to_string(),
        });
    }

    fn insert_interrupting_session(
        sessions: &Arc<Mutex<SessionRegistry>>,
        id: &str,
        exec_id: &str,
    ) {
        let mut reg = sessions.lock().unwrap();
        reg.insert(SessionRecord {
            id: id.to_string(),
            label: "Test".to_string(),
            cwd: "/home/user".to_string(),
            shell: "bash".to_string(),
            status: "active".to_string(),
            pty_pair: make_dummy_pty_pair(),
            writer: make_dummy_writer(),
            pending_execution_id: Some(exec_id.to_string()),
            exec_state: SessionExecState::Interrupting,
            boot_prompt_received: true,
            command_sent_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_active_at: "2026-01-01T00:00:00Z".to_string(),
        });
    }

    fn prompt_marker() -> &'static str {
        crate::pty::PROMPT_MARKER
    }

    // ---- Boot sequence parity ----

    #[test]
    fn parity_boot_emits_ready_with_cwd() {
        let sink = make_sink();
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = make_sessions();
        insert_booting_session(&sessions, "s1");

        let marker = format!("{}|/home/user|0\n", prompt_marker());
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);

        let events = sink.events();
        assert_eq!(events.len(), 3, "Boot should emit 3 events: CwdChanged, SessionReady, ExecStateChanged");

        assert!(matches!(&events[0], RuntimeEvent::SessionCwdChanged(e) if e.cwd == "/home/user"));
        assert!(matches!(&events[1], RuntimeEvent::SessionReady(e) if e.cwd == "/home/user" && e.session_id == "s1"));
        assert!(matches!(&events[2], RuntimeEvent::SessionExecStateChanged(e) if e.exec_state == "ready"));
    }

    // ---- Execution completion parity ----

    #[test]
    fn parity_execution_success_emits_finished_with_exit_0() {
        let sink = make_sink();
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = make_sessions();
        insert_running_session(&sessions, "s1", "exec-1");

        let marker = format!("{}|/tmp|0\n", prompt_marker());
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);

        let events = sink.events();
        let finished = events.iter().find(|e| matches!(e, RuntimeEvent::ExecutionFinished(_)));
        assert!(finished.is_some(), "Must emit ExecutionFinished");

        if let RuntimeEvent::ExecutionFinished(e) = finished.unwrap() {
            assert_eq!(e.exit_code, 0);
            assert_eq!(e.status, "success");
            assert_eq!(e.session_id, "s1");
        }
    }

    #[test]
    fn parity_execution_failure_preserves_exit_code() {
        let sink = make_sink();
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = make_sessions();
        insert_running_session(&sessions, "s1", "exec-2");

        let marker = format!("{}|/tmp|1\n", prompt_marker());
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);

        let events = sink.events();
        let finished = events.iter().find(|e| matches!(e, RuntimeEvent::ExecutionFinished(_)));
        assert!(finished.is_some());

        if let RuntimeEvent::ExecutionFinished(e) = finished.unwrap() {
            assert_eq!(e.exit_code, 1);
            assert_eq!(e.status, "failure");
        }
    }

    // ---- Interrupt completion parity ----

    #[test]
    fn parity_interrupt_completion_exit_130() {
        let sink = make_sink();
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = make_sessions();
        insert_interrupting_session(&sessions, "s1", "exec-3");

        let marker = format!("{}|/home/user|130\n", prompt_marker());
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);

        let events = sink.events();
        let finished = events.iter().find(|e| matches!(e, RuntimeEvent::ExecutionFinished(_)));
        assert!(finished.is_some());

        if let RuntimeEvent::ExecutionFinished(e) = finished.unwrap() {
            assert_eq!(e.exit_code, 130);
            assert_eq!(e.status, "interrupted");
        }
    }

    // ---- CWD change parity ----

    #[test]
    fn parity_cwd_change_emits_event() {
        let sink = make_sink();
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = make_sessions();
        insert_running_session(&sessions, "s1", "exec-4");

        let marker = format!("{}|/new/dir|0\n", prompt_marker());
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", &marker);

        let events = sink.events();
        let cwd_event = events.iter().find(|e| matches!(e, RuntimeEvent::SessionCwdChanged(_)));
        assert!(cwd_event.is_some());

        if let RuntimeEvent::SessionCwdChanged(e) = cwd_event.unwrap() {
            assert_eq!(e.cwd, "/new/dir");
        }
    }

    // ---- Terminal output parity ----

    #[test]
    fn parity_output_emits_terminal_line_with_session_id() {
        let sink = make_sink();
        let sink_dyn: Arc<dyn RuntimeEventSink> = sink.clone();
        let sessions = make_sessions();

        // Plain text (no session record needed for line events)
        SessionService::process_reader_chunk(&sink_dyn, &sessions, "s1", "hello world\n");

        let events = sink.events();
        assert_eq!(events.len(), 1);

        if let RuntimeEvent::TerminalLine(e) = &events[0] {
            assert_eq!(e.session_id, "s1");
            assert!(e.text.contains("hello world"));
            assert_eq!(e.kind, "stdout");
        } else {
            panic!("Expected TerminalLine event");
        }
    }
}
