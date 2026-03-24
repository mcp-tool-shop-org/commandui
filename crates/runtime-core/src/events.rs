//! Canonical runtime event contracts.
//!
//! These types define the semantic events produced by the runtime.
//! Both Desktop (Tauri) and Console (Ratatui) adapters consume these.
//! No adapter-specific types belong here.

use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLineEvent {
    pub id: String,
    pub session_id: String,
    pub execution_id: Option<String>,
    pub kind: String,
    pub text: String,
    pub timestamp: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCwdChangedEvent {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFinishedEvent {
    pub execution_id: String,
    pub session_id: String,
    pub exit_code: i32,
    pub finished_at: String,
    pub status: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReadyEvent {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExecStateChangedEvent {
    pub session_id: String,
    pub exec_state: String,
    pub changed_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSummary {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub source: String,
    pub linked_plan_id: Option<String>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStartedEvent {
    pub execution: ExecutionSummary,
}

// --- Event sink abstraction ---

/// Canonical envelope for all runtime events.
///
/// Adapters (Desktop/Console) receive these and translate them to
/// their own transport (Tauri emits, Ratatui app messages, etc.).
#[derive(Clone)]
pub enum RuntimeEvent {
    TerminalLine(TerminalLineEvent),
    SessionReady(SessionReadyEvent),
    SessionCwdChanged(SessionCwdChangedEvent),
    SessionExecStateChanged(SessionExecStateChangedEvent),
    ExecutionStarted(ExecutionStartedEvent),
    ExecutionFinished(ExecutionFinishedEvent),
}

/// Abstract sink for runtime events.
///
/// The runtime produces semantic events through this trait.
/// Each shell adapter implements it to deliver events in its own way.
///
/// Must be Send + Sync because the PTY reader loop runs in a spawned thread.
pub trait RuntimeEventSink: Send + Sync {
    fn emit(&self, event: RuntimeEvent);
}

/// No-op sink for tests and headless operation.
pub struct NoopSink;

impl RuntimeEventSink for NoopSink {
    fn emit(&self, _event: RuntimeEvent) {}
}

/// Collecting sink for tests — captures all emitted events.
#[cfg(any(test, feature = "test-utils"))]
pub struct CollectingSink {
    events: std::sync::Mutex<Vec<RuntimeEvent>>,
}

#[cfg(any(test, feature = "test-utils"))]
impl CollectingSink {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<RuntimeEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl RuntimeEventSink for CollectingSink {
    fn emit(&self, event: RuntimeEvent) {
        self.events.lock().unwrap().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_sink_accepts_all_events() {
        let sink = NoopSink;
        sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
            session_id: "s1".to_string(),
            cwd: "/tmp".to_string(),
        }));
        sink.emit(RuntimeEvent::TerminalLine(TerminalLineEvent {
            id: "l1".to_string(),
            session_id: "s1".to_string(),
            execution_id: None,
            kind: "stdout".to_string(),
            text: "hello".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        }));
        // NoopSink never panics — that is the test.
    }

    #[test]
    fn test_collecting_sink_captures_events() {
        let sink = CollectingSink::new();

        sink.emit(RuntimeEvent::SessionCwdChanged(SessionCwdChangedEvent {
            session_id: "s1".to_string(),
            cwd: "/home".to_string(),
        }));

        sink.emit(RuntimeEvent::SessionExecStateChanged(
            SessionExecStateChangedEvent {
                session_id: "s1".to_string(),
                exec_state: "ready".to_string(),
                changed_at: "2026-01-01T00:00:00Z".to_string(),
            },
        ));

        sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
            execution_id: "e1".to_string(),
            session_id: "s1".to_string(),
            exit_code: 0,
            finished_at: "2026-01-01T00:00:01Z".to_string(),
            status: "success".to_string(),
        }));

        assert_eq!(sink.len(), 3);

        let events = sink.events();
        assert!(matches!(events[0], RuntimeEvent::SessionCwdChanged(_)));
        assert!(matches!(events[1], RuntimeEvent::SessionExecStateChanged(_)));
        assert!(matches!(events[2], RuntimeEvent::ExecutionFinished(_)));
    }

    #[test]
    fn test_sink_is_send_sync() {
        // Compile-time proof that the trait bounds are correct.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NoopSink>();
        assert_send_sync::<CollectingSink>();
    }

    #[test]
    fn test_collecting_sink_is_thread_safe() {
        use std::sync::Arc;
        use std::thread;

        let sink = Arc::new(CollectingSink::new());
        let mut handles = vec![];

        for i in 0..5 {
            let sink_clone = sink.clone();
            handles.push(thread::spawn(move || {
                sink_clone.emit(RuntimeEvent::TerminalLine(TerminalLineEvent {
                    id: format!("l{i}"),
                    session_id: "s1".to_string(),
                    execution_id: None,
                    kind: "stdout".to_string(),
                    text: format!("line {i}"),
                    timestamp: "2026-01-01T00:00:00Z".to_string(),
                }));
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(sink.len(), 5);
    }
}
