use commandui_runtime_core::events::RuntimeEventSink;
use commandui_runtime_core::services::session_service::SessionService;
use commandui_runtime_core::services::terminal_service::TerminalService;
use commandui_runtime_core::session::SessionRegistry;
use commandui_runtime_planner::OllamaConfig;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub sessions: Arc<Mutex<SessionRegistry>>,
    pub db_path: Arc<Mutex<Option<PathBuf>>>,
    pub ollama: OllamaConfig,
    pub event_sink: Arc<dyn RuntimeEventSink>,
    pub session_service: SessionService,
    pub terminal_service: TerminalService,
}

impl AppState {
    pub fn new(event_sink: Arc<dyn RuntimeEventSink>) -> Self {
        let sessions = Arc::new(Mutex::new(SessionRegistry::new()));

        let session_service = SessionService::new(sessions.clone(), event_sink.clone());
        let terminal_service = TerminalService::new(sessions.clone(), event_sink.clone());

        Self {
            sessions,
            db_path: Arc::new(Mutex::new(None)),
            ollama: OllamaConfig::default(),
            event_sink,
            session_service,
            terminal_service,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commandui_runtime_core::events::NoopSink;

    #[test]
    fn new_starts_without_database_or_sessions() {
        let sink: Arc<dyn RuntimeEventSink> = Arc::new(NoopSink);
        let state = AppState::new(sink);
        assert!(state.db_path.lock().expect("db path").is_none());
        assert!(state.sessions.lock().expect("sessions").list().is_empty());
    }
}
