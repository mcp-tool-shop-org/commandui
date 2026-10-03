//! Live shell tests: drive real shells through SessionService and
//! TerminalService. Every unit test in the crate asserts bytes; these prove
//! that a real shell actually becomes ready, runs a command, discards
//! half-typed input, reports failure, and reports its cwd.
//!
//! A shell that is not installed on this machine is skipped with a message on
//! stderr, never silently.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use commandui_runtime_core::events::{RuntimeEvent, RuntimeEventSink};
use commandui_runtime_core::services::session_service::{CreateSessionRequest, SessionService};
use commandui_runtime_core::services::terminal_service::{ExecuteRequest, TerminalService};
use commandui_runtime_core::session::SessionRegistry;

const TIMEOUT: Duration = Duration::from_secs(30);

struct Collect(Mutex<Vec<RuntimeEvent>>);

impl RuntimeEventSink for Collect {
    fn emit(&self, event: RuntimeEvent) {
        self.0.lock().unwrap().push(event);
    }
}

struct Live {
    sink: Arc<Collect>,
    sessions: SessionService,
    terminal: TerminalService,
    id: String,
    n: u32,
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.sessions.close(&self.id);
    }
}

fn wait_for<F: Fn(&[RuntimeEvent]) -> bool>(sink: &Collect, f: F) -> bool {
    let end = Instant::now() + TIMEOUT;
    while Instant::now() < end {
        if f(&sink.0.lock().unwrap()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// What a terminal would show: escape sequences dropped, a cursor-position
/// escape starts a new row, a carriage return overwrites the row so far.
fn visible_lines(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut rows: Vec<String> = vec![String::new()];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\u{1b}' => {
                i += 1;
                match chars.get(i) {
                    Some('[') => {
                        i += 1;
                        while i < chars.len() && ('\u{20}'..='\u{3f}').contains(&chars[i]) {
                            i += 1;
                        }
                        if matches!(chars.get(i), Some('H') | Some('f')) {
                            rows.push(String::new());
                        }
                    }
                    Some(']') => {
                        i += 1;
                        while i < chars.len() && chars[i] != '\u{7}' {
                            i += 1;
                        }
                    }
                    _ => {}
                }
            }
            '\r' => {
                if chars.get(i + 1) != Some(&'\n') {
                    rows.last_mut().unwrap().clear();
                }
            }
            '\n' => rows.push(String::new()),
            '\u{7}' => {}
            _ => rows.last_mut().unwrap().push(c),
        }
        i += 1;
    }
    rows.into_iter().map(|r| r.trim().to_string()).collect()
}

fn output_lines(sink: &Collect) -> Vec<String> {
    let text: String = sink
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            RuntimeEvent::TerminalLine(l) => Some(l.text.clone()),
            _ => None,
        })
        .collect();
    visible_lines(&text)
}

fn work_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("commandui-live-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    dir
}

fn start(shell: &str, cwd: &Path) -> Live {
    let sink = Arc::new(Collect(Mutex::new(Vec::new())));
    let registry = Arc::new(Mutex::new(SessionRegistry::new()));
    let sessions = SessionService::new(registry.clone(), sink.clone());
    let terminal = TerminalService::new(registry, sink.clone());
    let summary = sessions
        .create(CreateSessionRequest {
            label: None,
            cwd: Some(cwd.to_string_lossy().to_string()),
            shell: Some(shell.to_string()),
        })
        .expect("create session");
    let id = summary.id;
    let ready = wait_for(&sink, |ev| {
        ev.iter()
            .any(|e| matches!(e, RuntimeEvent::SessionReady(r) if r.session_id == id))
    });
    if !ready {
        let lines = output_lines(&sink);
        let _ = sessions.close(&id);
        panic!("{shell}: session never became ready; output: {lines:?}");
    }
    // Let the shell finish drawing its first prompt.
    std::thread::sleep(Duration::from_millis(800));
    Live { sink, sessions, terminal, id, n: 0 }
}

impl Live {
    /// Run a command; returns its exit code and the visible output lines it
    /// produced.
    fn run(&mut self, command: &str) -> (i32, Vec<String>) {
        self.n += 1;
        let exec_id = format!("live-{}", self.n);
        let before = self.sink.0.lock().unwrap().len();
        self.terminal
            .execute(ExecuteRequest {
                execution_id: exec_id.clone(),
                session_id: self.id.clone(),
                command: command.to_string(),
                source: "live-test".to_string(),
                linked_plan_id: None,
            })
            .expect("execute");
        let done = wait_for(&self.sink, |ev| {
            ev.iter().any(|e| matches!(e, RuntimeEvent::ExecutionFinished(f) if f.execution_id == exec_id))
        });
        assert!(done, "`{command}` never finished; output: {:?}", output_lines(&self.sink));
        let events = self.sink.0.lock().unwrap();
        let exit = events
            .iter()
            .find_map(|e| match e {
                RuntimeEvent::ExecutionFinished(f) if f.execution_id == exec_id => Some(f.exit_code),
                _ => None,
            })
            .unwrap();
        let text: String = events[before..]
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(l) => Some(l.text.clone()),
                _ => None,
            })
            .collect();
        (exit, visible_lines(&text))
    }

    fn last_cwd(&self) -> String {
        self.sink
            .0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|e| match e {
                RuntimeEvent::SessionCwdChanged(c) if c.session_id == self.id => Some(c.cwd.clone()),
                _ => None,
            })
            .expect("a cwd was reported")
    }
}

fn last_component(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// The whole contract, against one real shell.
fn exercise(shell: &str, tag: &str) {
    let dir = work_dir(tag);
    let mut live = start(shell, &dir);

    // Ready, and the cwd it reported is the one we asked for.
    let folder = last_component(&dir.to_string_lossy());
    assert_eq!(last_component(&live.last_cwd()), folder, "{shell}: ready cwd");

    // A command finishes with exit 0 and a bare output line.
    let (exit, lines) = live.run("echo probe-ok");
    assert_eq!(exit, 0, "{shell}: echo exit; lines {lines:?}");
    assert!(lines.iter().any(|l| l == "probe-ok"), "{shell}: no bare probe-ok line in {lines:?}");

    // Half-typed input is discarded: nothing glued to the command and the
    // typed text is never run.
    live.terminal.write(&live.id, "xyz").expect("type half a line");
    std::thread::sleep(Duration::from_millis(500));
    let (exit, lines) = live.run("echo probe-two");
    assert_eq!(exit, 0, "{shell}: exit after half-typed input; lines {lines:?}");
    assert!(lines.iter().any(|l| l == "probe-two"), "{shell}: no bare probe-two line in {lines:?}");
    let joined = lines.join("\n").to_lowercase();
    assert!(!joined.contains("xyzecho"), "{shell}: typed text glued to the command: {lines:?}");
    assert!(!joined.contains("not recognized"), "{shell}: typed text was run: {lines:?}");
    assert!(!joined.contains("not found"), "{shell}: typed text was run: {lines:?}");
    assert!(!joined.contains("xyz"), "{shell}: typed text survived the clear: {lines:?}");

    // A failing command reports a non-zero exit.
    let (exit, lines) = live.run("commandui_no_such_command_zz");
    assert_ne!(exit, 0, "{shell}: failing command reported success; lines {lines:?}");

    // The shell is still usable after a failure, and cwd follows `cd`.
    let (exit, lines) = live.run("cd sub");
    assert_eq!(exit, 0, "{shell}: cd exit; lines {lines:?}");
    assert_eq!(last_component(&live.last_cwd()), "sub", "{shell}: cwd after cd");
    let (exit, _) = live.run("echo probe-three");
    assert_eq!(exit, 0, "{shell}: exit after cd");

    drop(live);
    let _ = std::fs::remove_dir_all(&dir);
}

fn first_existing(candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| p.to_string())
}

/// Resolve a program on PATH (an App Execution Alias such as the Store pwsh
/// is a file in PATH, so a PATH walk finds it).
#[cfg(windows)]
fn on_path(exe: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(exe))
        .find(|p| std::fs::metadata(p).is_ok())
        .map(|p| p.to_string_lossy().to_string())
}

fn skip(shell: &str, why: &str) {
    eprintln!("SKIPPED live shell test for {shell}: {why}");
}

#[cfg(windows)]
#[test]
fn live_cmd() {
    exercise("cmd.exe", "cmd");
}

#[cfg(windows)]
#[test]
fn live_windows_powershell() {
    exercise("powershell.exe", "ps5");
}

#[cfg(windows)]
#[test]
fn live_pwsh() {
    match on_path("pwsh.exe") {
        Some(path) => exercise(&path, "pwsh"),
        None => skip("pwsh.exe", "not on PATH"),
    }
}

#[cfg(windows)]
#[test]
fn live_git_bash_on_windows() {
    match first_existing(&[
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ]) {
        Some(path) => exercise(&path, "gitbash"),
        None => skip("Git Bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_bash() {
    match first_existing(&["/bin/bash", "/usr/bin/bash"]) {
        Some(path) => exercise(&path, "bash"),
        None => skip("bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_zsh() {
    match first_existing(&["/bin/zsh", "/usr/bin/zsh", "/opt/homebrew/bin/zsh"]) {
        Some(path) => exercise(&path, "zsh"),
        None => skip("zsh", "not installed"),
    }
}

#[test]
fn visible_lines_follows_a_terminal() {
    assert_eq!(visible_lines("a\r\nb\rc\r\n"), vec!["a", "c", ""]);
    assert_eq!(
        visible_lines("\u{1b}[?25lone\u{1b}[5;1Htwo\u{1b}]0;title\u{7}\r\n"),
        vec!["one", "two", ""]
    );
}

#[cfg(windows)]
#[test]
fn default_shell_prefers_pwsh_on_path_over_windows_powershell() {
    if std::env::var_os("COMMANDUI_WINDOWS_SHELL").is_some() {
        return skip("default_shell", "COMMANDUI_WINDOWS_SHELL overrides it");
    }
    let Some(on_path_pwsh) = on_path("pwsh.exe") else {
        return skip("default_shell", "pwsh.exe is not on PATH, so 5.1 is the right answer");
    };
    let chosen = commandui_runtime_core::pty::default_shell();
    assert!(chosen.to_lowercase().ends_with("pwsh.exe"), "chose {chosen}, pwsh is at {on_path_pwsh}");
}
