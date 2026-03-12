use crate::shell::pty_manager::PtyHandle;
use portable_pty::{PtyPair, PtySize};
use std::collections::HashMap;

pub struct SessionRecord {
    pub id: String,
    pub label: String,
    pub cwd: String,
    pub shell: String,
    pub status: String,
    pub pty_pair: PtyPair,
    pub writer: PtyHandle,
    pub pending_execution_id: Option<String>,
    pub created_at: String,
    pub last_active_at: String,
}

pub struct SessionRegistry {
    sessions: HashMap<String, SessionRecord>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn insert(&mut self, record: SessionRecord) {
        self.sessions.insert(record.id.clone(), record);
    }

    pub fn get(&self, session_id: &str) -> Option<&SessionRecord> {
        self.sessions.get(session_id)
    }

    pub fn get_mut(&mut self, session_id: &str) -> Option<&mut SessionRecord> {
        self.sessions.get_mut(session_id)
    }

    pub fn remove(&mut self, session_id: &str) -> Option<SessionRecord> {
        self.sessions.remove(session_id)
    }

    pub fn list(&self) -> Vec<&SessionRecord> {
        self.sessions.values().collect()
    }

    pub fn resize(&mut self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let record = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;

        record
            .pty_pair
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Resize error: {e}"))?;

        Ok(())
    }

    pub fn set_pending_execution(
        &mut self,
        session_id: &str,
        execution_id: Option<String>,
    ) -> Result<(), String> {
        let record = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;
        record.pending_execution_id = execution_id;
        Ok(())
    }

    pub fn pending_execution_id(&self, session_id: &str) -> Option<String> {
        self.sessions
            .get(session_id)
            .and_then(|r| r.pending_execution_id.clone())
    }
}
