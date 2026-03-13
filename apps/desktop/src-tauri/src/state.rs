use crate::shell::session_registry::SessionRegistry;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct OllamaConfig {
    pub endpoint: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "qwen2.5:14b".to_string(),
            timeout_secs: 30,
        }
    }
}

pub struct AppState {
    pub sessions: Arc<Mutex<SessionRegistry>>,
    pub db_path: Arc<Mutex<Option<PathBuf>>>,
    pub ollama: OllamaConfig,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(SessionRegistry::new())),
            db_path: Arc::new(Mutex::new(None)),
            ollama: OllamaConfig::default(),
        }
    }
}
