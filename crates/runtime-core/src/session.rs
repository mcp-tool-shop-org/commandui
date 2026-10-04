use crate::pty::PtyHandle;
use portable_pty::{PtyPair, PtySize};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionExecState {
    Booting,
    Ready,
    Running,
    Interrupting,
    Desynced,
    /// A command the user typed by hand is running (ssh, vim, make): the
    /// session is not at a prompt, so nothing may be typed into it for them.
    UserRunning,
}

impl std::fmt::Display for SessionExecState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Booting => write!(f, "booting"),
            Self::Ready => write!(f, "ready"),
            Self::Running => write!(f, "running"),
            Self::Interrupting => write!(f, "interrupting"),
            Self::Desynced => write!(f, "desynced"),
            Self::UserRunning => write!(f, "userRunning"),
        }
    }
}

/// Per-session bookkeeping between the writers (execute, write, resync) and
/// the reader. None of it is shell output; the shell's markers are invisible
/// OSC sequences and nothing here depends on lines or on the console width.
pub struct SessionTracking {
    /// The exit code a cmd exit marker (`X`) reported for the execution that
    /// is pending. cmd's prompt cannot expand ERRORLEVEL, so the command line
    /// carries a tail that reports it just before the prompt is drawn.
    pub pending_exit: Option<i32>,
    /// How the pending cmd execution was written (see `crate::pty::CmdTail`).
    pub pending_tail: crate::pty::CmdTail,
    /// The runtime already sent the follow-up probe for a pending cmd
    /// execution that had no tail.
    pub probed: bool,
    /// Prompts still owed to lines the user typed or pasted while the session
    /// was Ready: a paste of two lines draws two prompts, and only the last
    /// one returns the session to Ready.
    pub user_prompts_owed: u32,
    /// Whether the shell reported its clear-line chord as bound. Until it
    /// says otherwise the chord is assumed to be there.
    pub clear_chord: bool,
    /// A tail of display text held back because it could be the start of cmd
    /// plumbing the console echoes (see `crate::pty::strip_cmd_plumbing`).
    pub display_hold: String,
    /// The console width the reader sizes its display rewrite for. The resize
    /// path updates it.
    pub width: std::sync::Arc<std::sync::atomic::AtomicU16>,
}

impl Default for SessionTracking {
    fn default() -> Self {
        Self {
            pending_exit: None,
            pending_tail: crate::pty::CmdTail::Chained,
            probed: false,
            user_prompts_owed: 0,
            clear_chord: true,
            display_hold: String::new(),
            width: std::sync::Arc::new(std::sync::atomic::AtomicU16::new(crate::pty::PTY_COLS)),
        }
    }
}

pub struct SessionRecord {
    pub id: String,
    pub label: String,
    pub cwd: String,
    pub shell: String,
    pub status: String,
    pub pty_pair: PtyPair,
    pub writer: PtyHandle,
    pub pending_execution_id: Option<String>,
    /// Per-session token the shell prompt echoes. It filters stale or unrelated
    /// output, but it is readable shell state (PROMPT_COMMAND, the prompt
    /// function, the echoed cmd line), so a command running in the session can
    /// still forge a completion marker. It is not a security boundary.
    pub marker_nonce: String,
    pub exec_state: SessionExecState,
    pub boot_prompt_received: bool,
    pub command_sent_at: Option<String>,
    /// Bumped by the reader on every prompt marker, so resync can tell that
    /// its probe was answered even if the state looks unchanged.
    pub marker_gen: u64,
    /// What the reader and the writers keep about the line discipline of the
    /// shell (see `SessionTracking`).
    pub track: SessionTracking,
    /// The shell process, kept so close() can kill and reap it.
    pub child: Option<crate::pty::ShellChild>,
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

    /// Sessions in a stable order: oldest first, id as the tie-break. HashMap
    /// iteration order changes between processes and rebuilds.
    pub fn list(&self) -> Vec<&SessionRecord> {
        let mut records: Vec<&SessionRecord> = self.sessions.values().collect();
        records.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
        records
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
        record.track.width.store(cols, std::sync::atomic::Ordering::Relaxed);

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

    pub fn set_exec_state(
        &mut self,
        session_id: &str,
        state: SessionExecState,
    ) -> Result<(), String> {
        let record = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;
        record.exec_state = state;
        Ok(())
    }

    pub fn exec_state(&self, session_id: &str) -> Option<SessionExecState> {
        self.sessions.get(session_id).map(|r| r.exec_state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_state_display() {
        assert_eq!(SessionExecState::Booting.to_string(), "booting");
        assert_eq!(SessionExecState::Ready.to_string(), "ready");
        assert_eq!(SessionExecState::Running.to_string(), "running");
        assert_eq!(SessionExecState::Interrupting.to_string(), "interrupting");
        assert_eq!(SessionExecState::Desynced.to_string(), "desynced");
    }

    #[test]
    fn test_exec_state_serialize() {
        let json = serde_json::to_string(&SessionExecState::Running).unwrap();
        assert_eq!(json, "\"running\"");
    }

    fn record(id: &str) -> SessionRecord {
        let pair = portable_pty::native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        SessionRecord {
            id: id.to_string(),
            label: format!("label-{id}"),
            cwd: "/work".into(),
            shell: "bash".into(),
            status: "active".into(),
            pty_pair: pair,
            writer: std::sync::Arc::new(std::sync::Mutex::new(
                Box::new(std::io::sink()) as Box<dyn std::io::Write + Send>
            )),
            pending_execution_id: None,
            marker_nonce: "nonce".into(),
            exec_state: SessionExecState::Booting,
            boot_prompt_received: false,
            command_sent_at: None,
            marker_gen: 0,
            track: SessionTracking::default(),
            child: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            last_active_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn registry_inserts_reads_updates_and_removes() {
        let mut registry = SessionRegistry::new();
        assert!(registry.get("s1").is_none());
        assert!(registry.pending_execution_id("s1").is_none());
        assert!(registry.exec_state("s1").is_none());
        assert!(registry.list().is_empty());

        registry.insert(record("s1"));
        registry.insert(record("s2"));
        assert_eq!(registry.list().len(), 2);
        assert_eq!(registry.get("s1").unwrap().label, "label-s1");
        registry.get_mut("s1").unwrap().label = "renamed".into();
        assert_eq!(registry.get("s1").unwrap().label, "renamed");

        registry
            .set_pending_execution("s1", Some("exec-1".into()))
            .unwrap();
        assert_eq!(registry.pending_execution_id("s1").as_deref(), Some("exec-1"));
        registry.set_pending_execution("s1", None).unwrap();
        assert!(registry.pending_execution_id("s1").is_none());

        registry
            .set_exec_state("s1", SessionExecState::Ready)
            .unwrap();
        assert_eq!(registry.exec_state("s1"), Some(SessionExecState::Ready));
        registry.resize("s1", 100, 40).unwrap();

        let removed = registry.remove("s1").unwrap();
        assert_eq!(removed.id, "s1");
        assert!(registry.get("s1").is_none());
        assert!(registry.remove("s1").is_none());
    }

    #[test]
    fn list_is_ordered_by_created_at_then_id() {
        let mut registry = SessionRegistry::new();
        for (id, created) in [
            ("s-c", "2026-01-01T00:00:03Z"),
            ("s-b", "2026-01-01T00:00:01Z"),
            ("s-z", "2026-01-01T00:00:02Z"),
            ("s-a", "2026-01-01T00:00:02Z"),
        ] {
            let mut rec = record(id);
            rec.created_at = created.into();
            registry.insert(rec);
        }
        let ids: Vec<&str> = registry.list().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["s-b", "s-a", "s-z", "s-c"]);
    }

    #[test]
    fn registry_missing_session_is_an_error() {
        let mut registry = SessionRegistry::new();
        for err in [
            registry.resize("missing", 80, 24).unwrap_err(),
            registry
                .set_pending_execution("missing", None)
                .unwrap_err(),
            registry
                .set_exec_state("missing", SessionExecState::Ready)
                .unwrap_err(),
        ] {
            assert!(err.contains("Session not found: missing"), "{err}");
        }
    }
}
