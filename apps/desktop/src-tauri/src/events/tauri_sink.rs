//! Desktop adapter: translates RuntimeEvent → Tauri emits.
//!
//! This is the only place in the desktop app that maps runtime
//! event semantics to Tauri transport. No business logic here.

use commandui_runtime_core::events::{RuntimeEvent, RuntimeEventSink};
use tauri::{AppHandle, Emitter, Runtime};

/// Tauri-backed implementation of RuntimeEventSink.
///
/// Holds an AppHandle and translates each RuntimeEvent variant
/// into the corresponding Tauri event name + payload.
///
/// The runtime parameter defaults to the desktop runtime. Tests pass the mock runtime.
pub struct TauriEventSink<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriEventSink<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> RuntimeEventSink for TauriEventSink<R> {
    fn emit(&self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::TerminalLine(e) => {
                let _ = self.app.emit("terminal:line", e);
            }
            RuntimeEvent::SessionReady(e) => {
                let _ = self.app.emit("session:ready", e);
            }
            RuntimeEvent::SessionCwdChanged(e) => {
                let _ = self.app.emit("session:cwd_changed", e);
            }
            RuntimeEvent::SessionExecStateChanged(e) => {
                let _ = self.app.emit("session:exec_state_changed", e);
            }
            RuntimeEvent::ExecutionStarted(e) => {
                let _ = self.app.emit("terminal:execution_started", e);
            }
            RuntimeEvent::ExecutionFinished(e) => {
                let _ = self.app.emit("terminal:execution_finished", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commandui_runtime_core::events::{
        ExecutionFinishedEvent, ExecutionStartedEvent, ExecutionSummary, RuntimeEvent,
        SessionCwdChangedEvent, SessionExecStateChangedEvent, SessionReadyEvent, TerminalLineEvent,
    };
    use std::sync::{Arc, Mutex};
    use tauri::Listener;

    #[test]
    fn emits_every_runtime_event() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let _webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview");

        let seen = Arc::new(Mutex::new(Vec::new()));
        for name in [
            "terminal:line",
            "session:ready",
            "session:cwd_changed",
            "session:exec_state_changed",
            "terminal:execution_started",
            "terminal:execution_finished",
        ] {
            let seen = seen.clone();
            let name = name.to_string();
            app.listen(name.clone(), move |event| {
                seen.lock().expect("seen").push((name.clone(), event.payload().to_string()));
            });
        }

        let sink = TauriEventSink::new(app.handle().clone());
        sink.emit(RuntimeEvent::TerminalLine(TerminalLineEvent {
            id: "line-1".into(),
            session_id: "s1".into(),
            execution_id: None,
            kind: "stdout".into(),
            text: "hi".into(),
            timestamp: "t0".into(),
        }));
        sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
            session_id: "s1".into(),
            cwd: "/work".into(),
        }));
        sink.emit(RuntimeEvent::SessionCwdChanged(SessionCwdChangedEvent {
            session_id: "s1".into(),
            cwd: "/next".into(),
        }));
        sink.emit(RuntimeEvent::SessionExecStateChanged(
            SessionExecStateChangedEvent {
                session_id: "s1".into(),
                exec_state: "ready".into(),
                changed_at: "t1".into(),
            },
        ));
        sink.emit(RuntimeEvent::ExecutionStarted(ExecutionStartedEvent {
            execution: ExecutionSummary {
                id: "e1".into(),
                session_id: "s1".into(),
                command: "echo hi".into(),
                source: "test".into(),
                linked_plan_id: None,
                status: "running".into(),
                started_at: "t2".into(),
                finished_at: None,
                exit_code: None,
            },
        }));
        sink.emit(RuntimeEvent::ExecutionFinished(ExecutionFinishedEvent {
            execution_id: "e1".into(),
            session_id: "s1".into(),
            exit_code: 0,
            finished_at: "t3".into(),
            status: "success".into(),
        }));

        let seen = seen.lock().expect("seen");
        assert_eq!(seen.len(), 6, "{seen:?}");
        assert!(seen.iter().any(|(n, p)| n == "terminal:line" && p.contains("line-1")));
        assert!(seen.iter().any(|(n, p)| n == "session:ready" && p.contains("/work")));
        assert!(seen.iter().any(|(n, p)| n == "session:cwd_changed" && p.contains("/next")));
        assert!(seen.iter().any(|(n, p)| n == "session:exec_state_changed" && p.contains("ready")));
        assert!(seen.iter().any(|(n, p)| n == "terminal:execution_started" && p.contains("echo hi")));
        assert!(seen
            .iter()
            .any(|(n, p)| n == "terminal:execution_finished" && p.contains("success")));
    }
}
