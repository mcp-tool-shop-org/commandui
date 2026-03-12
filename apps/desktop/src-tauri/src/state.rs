use crate::shell::session_registry::SessionRegistry;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub sessions: Arc<Mutex<SessionRegistry>>,
    pub db_path: Arc<Mutex<Option<PathBuf>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(SessionRegistry::new())),
            db_path: Arc::new(Mutex::new(None)),
        }
    }
}
