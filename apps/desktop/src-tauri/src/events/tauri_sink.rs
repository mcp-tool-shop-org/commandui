//! Desktop adapter: translates RuntimeEvent → Tauri emits.
//!
//! This is the only place in the desktop app that maps runtime
//! event semantics to Tauri transport. No business logic here.

use commandui_runtime_core::events::{RuntimeEvent, RuntimeEventSink};
use tauri::{AppHandle, Emitter};

/// Tauri-backed implementation of RuntimeEventSink.
///
/// Holds an AppHandle and translates each RuntimeEvent variant
/// into the corresponding Tauri event name + payload.
pub struct TauriEventSink {
    app: AppHandle,
}

impl TauriEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl RuntimeEventSink for TauriEventSink {
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
