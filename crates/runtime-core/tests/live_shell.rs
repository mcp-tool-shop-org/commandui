//! Live shell tests: drive real shells through SessionService and
//! TerminalService. Every unit test in the crate asserts bytes; these prove
//! that a real shell actually becomes ready, runs a command, discards
//! half-typed input in every editing mode, reports its exact exit code,
//! survives hostile output, can be interrupted and resynced, reports that a
//! command the user typed is running, and reports when it exits.
//!
//! A shell that is not installed on this machine is skipped with a message on
//! stderr, never silently, unless it is required (CI): `COMMANDUI_REQUIRE_SHELLS=1`
//! requires every shell, and a comma-separated list (`bash,cmd`) requires just
//! those (`cmd`, `powershell`, `pwsh`, `gitbash`, `bash`, `zsh`), so a job that
//! only has some of them can still insist on the ones it has.
//! A required shell that is missing fails the test.
//!
//! The tests take a lock so one real shell runs at a time: a shell that has to
//! start while three others are starting (or while the machine is busy) is slow
//! to draw its first prompt, and what these tests measure is correctness, not
//! start-up under contention. A shell that needs more than `READY_TIMEOUT` to
//! become ready fails the test with the output it did produce.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use commandui_runtime_core::events::{RuntimeEvent, RuntimeEventSink};
use commandui_runtime_core::services::session_service::{CreateSessionRequest, SessionService};
use commandui_runtime_core::services::terminal_service::{
    ExecuteRequest, TerminalService, USER_RUNNING_ERROR,
};
use commandui_runtime_core::session::SessionRegistry;

/// A shell may take this long to draw its first prompt on a loaded machine.
const READY_TIMEOUT: Duration = Duration::from_secs(90);
/// A command may take this long to finish once the shell is ready.
const TIMEOUT: Duration = Duration::from_secs(45);

static ONE_SHELL_AT_A_TIME: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Cmd,
    PowerShell,
    Bash,
    /// zsh: runs the bash command table, without the editing-mode steps (its
    /// vi keymap is not exercised here).
    #[cfg_attr(windows, allow(dead_code))]
    Zsh,
}

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
    kind: Kind,
    shell: String,
    /// Index of the SessionReady event: what the user sees from here on is
    /// not the shell's banner or the echo of the bootstrap.
    ready_at: usize,
    _guard: MutexGuard<'static, ()>,
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.sessions.close(&self.id);
    }
}

fn wait_until<F: Fn() -> bool>(limit: Duration, f: F) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn wait_for<F: Fn(&[RuntimeEvent]) -> bool>(sink: &Collect, limit: Duration, f: F) -> bool {
    wait_until(limit, || f(&sink.0.lock().unwrap()))
}

/// What a terminal would show: escape sequences dropped (a cursor-position
/// escape starts a new row; erase-in-line `K`, delete-character `P` and
/// erase-character `X` are applied), a carriage return moves the cursor to the
/// start of the row and what is written next overwrites from there (the tail of
/// the row stays until it is written over), and a backspace moves the cursor
/// left so that what is written next overwrites (bash redraws a cleared line as
/// backspaces, spaces, backspaces: the text it erased is not on screen).
fn visible_lines(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut rows: Vec<Vec<char>> = vec![Vec::new()];
    let mut cursor = 0usize;
    let mut i = 0;
    let put = |rows: &mut Vec<Vec<char>>, cursor: &mut usize, c: char| {
        let row = rows.last_mut().unwrap();
        while row.len() < *cursor {
            row.push(' ');
        }
        if *cursor < row.len() {
            row[*cursor] = c;
        } else {
            row.push(c);
        }
        *cursor += 1;
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\u{1b}' => {
                i += 1;
                match chars.get(i) {
                    Some('[') => {
                        let start = i + 1;
                        i += 1;
                        while i < chars.len() && ('\u{20}'..='\u{3f}').contains(&chars[i]) {
                            i += 1;
                        }
                        let params: String = chars[start..i.min(chars.len())].iter().collect();
                        let n = params.split(';').next().and_then(|p| p.parse::<usize>().ok());
                        match chars.get(i) {
                            Some('H') | Some('f') => {
                                rows.push(Vec::new());
                                cursor = 0;
                            }
                            Some('C') => cursor += n.unwrap_or(1),
                            Some('D') => cursor = cursor.saturating_sub(n.unwrap_or(1)),
                            Some('K') => {
                                let row = rows.last_mut().unwrap();
                                match n.unwrap_or(0) {
                                    0 => row.truncate(cursor),
                                    1 => {
                                        for cell in row.iter_mut().take(cursor + 1) {
                                            *cell = ' ';
                                        }
                                    }
                                    _ => row.clear(),
                                }
                            }
                            Some('P') => {
                                let row = rows.last_mut().unwrap();
                                let count = n.unwrap_or(1);
                                if cursor < row.len() {
                                    let end = (cursor + count).min(row.len());
                                    row.drain(cursor..end);
                                }
                            }
                            Some('X') => {
                                let row = rows.last_mut().unwrap();
                                let count = n.unwrap_or(1);
                                for at in cursor..cursor + count {
                                    if at < row.len() {
                                        row[at] = ' ';
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(']') => {
                        i += 1;
                        // An OSC ends at BEL or at ESC \ (cmd's CmdNotFound
                        // notice is terminated with the latter).
                        while i < chars.len()
                            && chars[i] != '\u{7}'
                            && !(chars[i] == '\u{1b}' && chars.get(i + 1) == Some(&'\\'))
                        {
                            i += 1;
                        }
                        if chars.get(i) == Some(&'\u{1b}') {
                            i += 1;
                        }
                    }
                    _ => {}
                }
            }
            '\r' => cursor = 0,
            '\n' => {
                cursor = 0;
                rows.push(Vec::new());
            }
            '\u{7}' => {}
            '\u{8}' => cursor = cursor.saturating_sub(1),
            _ => put(&mut rows, &mut cursor, c),
        }
        i += 1;
    }
    rows.into_iter().map(|r| r.into_iter().collect::<String>().trim().to_string()).collect()
}

fn lines_since(sink: &Collect, from: usize) -> Vec<String> {
    let text: String = sink.0.lock().unwrap()[from..]
        .iter()
        .filter_map(|e| match e {
            RuntimeEvent::TerminalLine(l) => Some(l.text.clone()),
            _ => None,
        })
        .collect();
    visible_lines(&text)
}

fn raw_text(sink: &Collect) -> String {
    sink.0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            RuntimeEvent::TerminalLine(l) => Some(l.text.clone()),
            _ => None,
        })
        .collect()
}

fn output_lines(sink: &Collect) -> Vec<String> {
    lines_since(sink, 0)
}

fn work_dir(tag: &str) -> PathBuf {
    // The cwd is deliberately wider than the PTY (120 columns): the prompt
    // marker carries the full cwd, so a marker row that the console wraps must
    // still parse. CI runners' own temp paths are long enough to hit this; a
    // developer's are not, so the test pads the path itself.
    let dir = std::env::temp_dir()
        .join(format!("commandui-live-{}-{tag}", std::process::id()))
        .join("a-deliberately-long-directory-name-so-the-working-directory-is-wider-than-the-terminal")
        .join("and-then-another-long-directory-name-to-push-the-prompt-marker-past-two-hundred-columns");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    dir
}

/// A working directory of 300 characters (Windows cannot have one of more than
/// about 250: the longest it can have, which is wider than any console).
fn long_work_dir(tag: &str) -> PathBuf {
    let target: usize = if cfg!(windows) { 245 } else { 300 };
    let mut dir = std::env::temp_dir().join(format!("commandui-long-{}-{tag}", std::process::id()));
    let mut n = 0;
    while dir.to_string_lossy().len() + 2 < target {
        let room = target - dir.to_string_lossy().len() - 1;
        let name = format!("segment{n:02}-{}", "x".repeat(40));
        let take = room.min(name.len()).max(1);
        dir = dir.join(&name[..take]);
        n += 1;
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(shell: &str, kind: Kind, cwd: &Path) -> Live {
    let guard = ONE_SHELL_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
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
    let ready = wait_for(&sink, READY_TIMEOUT, |ev| {
        ev.iter()
            .any(|e| matches!(e, RuntimeEvent::SessionReady(r) if r.session_id == id))
    });
    if !ready {
        let lines = output_lines(&sink);
        let _ = sessions.close(&id);
        panic!("{shell}: session never became ready within {READY_TIMEOUT:?}; output: {lines:?}");
    }
    // Let the shell finish drawing its first prompt.
    std::thread::sleep(Duration::from_millis(800));
    let ready_at = sink
        .0
        .lock()
        .unwrap()
        .iter()
        .position(|e| matches!(e, RuntimeEvent::SessionReady(r) if r.session_id == id))
        .expect("a SessionReady event");
    Live {
        sink,
        sessions,
        terminal,
        id,
        n: 0,
        kind,
        shell: shell.to_string(),
        ready_at,
        _guard: guard,
    }
}

struct Finished {
    exit: i32,
    status: String,
}

impl Live {
    fn event_count(&self) -> usize {
        self.sink.0.lock().unwrap().len()
    }

    fn try_start(&mut self, command: &str) -> (String, Result<(), String>) {
        self.n += 1;
        let exec_id = format!("live-{}", self.n);
        let result = self
            .terminal
            .execute(ExecuteRequest {
                execution_id: exec_id.clone(),
                session_id: self.id.clone(),
                command: command.to_string(),
                source: "live-test".to_string(),
                linked_plan_id: None,
            })
            .map(|_| ());
        (exec_id, result)
    }

    fn start_command(&mut self, command: &str) -> String {
        let (exec_id, result) = self.try_start(command);
        result.expect("execute");
        exec_id
    }

    /// Wait for the execution to finish; asserts it finished exactly once.
    fn finish(&self, exec_id: &str, command: &str) -> Finished {
        let done = wait_for(&self.sink, TIMEOUT, |ev| {
            ev.iter()
                .any(|e| matches!(e, RuntimeEvent::ExecutionFinished(f) if f.execution_id == exec_id))
        });
        assert!(
            done,
            "{}: `{command}` never finished; output: {:?}",
            self.shell,
            output_lines(&self.sink)
        );
        // A duplicate finish would arrive from a second marker; give it a moment.
        std::thread::sleep(Duration::from_millis(300));
        let events = self.sink.0.lock().unwrap();
        let finished: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::ExecutionFinished(f) if f.execution_id == exec_id => {
                    Some(Finished { exit: f.exit_code, status: f.status.clone() })
                }
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1, "{}: `{command}` finished {} times", self.shell, finished.len());
        finished.into_iter().next().unwrap()
    }

    /// Run a command; returns its exit code and the visible output lines it
    /// produced.
    fn run(&mut self, command: &str) -> (i32, Vec<String>) {
        let before = self.event_count();
        let exec_id = self.start_command(command);
        let finished = self.finish(&exec_id, command);
        (finished.exit, lines_since(&self.sink, before))
    }

    /// Like `run`, without the pause that looks for a duplicate finish: for the
    /// commands of a long sequence (the duplicate check is made at the end).
    fn run_fast(&mut self, command: &str) -> (i32, Vec<String>) {
        let before = self.event_count();
        let exec_id = self.start_command(command);
        let done = wait_for(&self.sink, TIMEOUT, |ev| {
            ev.iter()
                .any(|e| matches!(e, RuntimeEvent::ExecutionFinished(f) if f.execution_id == exec_id))
        });
        assert!(done, "{}: `{command}` never finished; output: {:?}", self.shell, output_lines(&self.sink));
        let exit = self
            .sink
            .0
            .lock()
            .unwrap()
            .iter()
            .find_map(|e| match e {
                RuntimeEvent::ExecutionFinished(f) if f.execution_id == exec_id => Some(f.exit_code),
                _ => None,
            })
            .unwrap();
        (exit, lines_since(&self.sink, before))
    }

    fn debug_events(&self, upto: usize) -> String {
        self.sink.0.lock().unwrap().iter().take(upto).enumerate().map(|(i, e)| match e {
            RuntimeEvent::TerminalLine(l) => format!("{i}: line {:?}", l.text.chars().take(50).collect::<String>()),
            RuntimeEvent::SessionReady(_) => format!("{i}: READY"),
            RuntimeEvent::SessionCwdChanged(_) => format!("{i}: cwd"),
            _ => format!("{i}: other"),
        }).collect::<Vec<_>>().join("
")
    }

    /// Everything the user was shown since the session became ready, raw.
    fn shown_since_ready(&self) -> String {
        self.sink.0.lock().unwrap()[self.ready_at..]
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::TerminalLine(l) if l.kind == "stdout" => Some(l.text.clone()),
                _ => None,
            })
            .collect()
    }

    fn state_events(&self, from: usize) -> Vec<String> {
        self.sink.0.lock().unwrap()[from..]
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::SessionExecStateChanged(s) if s.session_id == self.id => {
                    Some(s.exec_state.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn wait_state(&self, from: usize, state: &str) {
        let ok = wait_until(TIMEOUT, || self.state_events(from).iter().any(|s| s == state));
        assert!(
            ok,
            "{}: never reached `{state}`; states {:?}; output: {:?}",
            self.shell,
            self.state_events(from),
            lines_since(&self.sink, from)
        );
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

    /// Type half a line, optionally followed by more keystrokes, wait for the
    /// shell to take it, then run an approved command. The typed text must be
    /// gone: not glued to the command, not run, not shown again.
    ///
    /// (The echo of the typed text before the clear is deliberately not
    /// scanned: ConPTY repaints a cleared PSReadLine line by cursor position,
    /// and `visible_lines` cannot model a screen, so a line that was erased
    /// would be reported as one that survived. What survives is what the
    /// approved command ran with, and that is checked below: the `xyz` and
    /// `xhalf` words, a not-recognized line, and the bare output word.)
    fn half_typed_then_run(&mut self, typed: &[&str], marker_word: &str, what: &str) {
        for part in typed {
            self.terminal.write(&self.id, part).expect("type");
            std::thread::sleep(Duration::from_millis(400));
        }
        std::thread::sleep(Duration::from_millis(300));
        let command = format!("echo {marker_word}");
        let (exit, lines) = self.run(&command);
        let joined = lines.join("\n").to_lowercase();
        assert_eq!(exit, 0, "{}: {what}: exit; lines {lines:?}", self.shell);
        assert!(
            lines.iter().any(|l| l == marker_word),
            "{}: {what}: no bare {marker_word} line in {lines:?}",
            self.shell
        );
        assert!(!joined.contains("xyz"), "{}: {what}: typed text survived the clear: {lines:?}", self.shell);
        assert!(!joined.contains("xhalf"), "{}: {what}: typed text survived the clear: {lines:?}", self.shell);
        assert!(!joined.contains("not recognized"), "{}: {what}: typed text was run: {lines:?}", self.shell);
        assert!(!joined.contains("not found"), "{}: {what}: typed text was run: {lines:?}", self.shell);
        assert!(!joined.contains("xyzecho"), "{}: {what}: typed text glued to the command: {lines:?}", self.shell);
    }

    fn long_command(&self) -> &'static str {
        match self.kind {
            Kind::Bash | Kind::Zsh => "sleep 20",
            _ => "ping -n 20 127.0.0.1",
        }
    }
}

fn last_component(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

fn exit_seven(kind: Kind) -> &'static str {
    match kind {
        Kind::Bash | Kind::Zsh => "bash -c 'exit 7'",
        _ => "cmd /c exit 7",
    }
}

fn thirty_lines(kind: Kind) -> &'static str {
    match kind {
        Kind::Cmd => "for /l %i in (1,1,30) do @echo line%i",
        Kind::PowerShell => "1..30 | % { \"line$_\" }",
        Kind::Bash | Kind::Zsh => "for i in $(seq 1 30); do echo line$i; done",
    }
}

/// Prints `CSI ? 1049 h` and never leaves the alternate screen, like a
/// full-screen program that was killed.
fn enter_alt_screen_forever(kind: Kind) -> &'static str {
    match kind {
        Kind::Cmd => "powershell -NoProfile -Command \"[Console]::Out.Write([string][char]27+'[?1049h'+[string][char]27+'[10;10Hscreen')\"",
        Kind::PowerShell => "[Console]::Out.Write([string][char]27+'[?1049h'+[string][char]27+'[10;10Hscreen')",
        Kind::Bash | Kind::Zsh => "printf '\\033[?1049h\\033[10;10Hscreen'",
    }
}

/// ESC followed by a non-ASCII character, then plain text.
fn escape_then_non_ascii(kind: Kind) -> &'static str {
    match kind {
        Kind::Cmd => "powershell -NoProfile -Command \"[Console]::Out.Write([string][char]27+[char]0xe9+'x')\"",
        Kind::PowerShell => "[Console]::Out.Write([string][char]27+[char]0xe9+'x')",
        Kind::Bash | Kind::Zsh => "printf '\\033\\303\\251x'",
    }
}

fn contains_in_order(lines: &[String], wanted: &[String]) -> bool {
    let mut at = 0;
    for want in wanted {
        match lines[at..].iter().position(|l| l == want) {
            Some(p) => at += p + 1,
            None => return false,
        }
    }
    true
}

/// A block of two lines as one write, the way a paste without bracketed paste
/// arrives (cmd, and Windows PowerShell without PSReadLine's 2004 mode).
fn pasted_block(kind: Kind) -> &'static str {
    match kind {
        Kind::Bash | Kind::Zsh => "sleep 1; echo pasted-A\rsleep 3; echo pasted-B\r",
        Kind::Cmd => "ping -n 2 127.0.0.1 >nul & echo pasted-A\rping -n 4 127.0.0.1 >nul & echo pasted-B\r",
        Kind::PowerShell => "Start-Sleep 1; 'pasted-A'\rStart-Sleep 3; 'pasted-B'\r",
    }
}

/// An approved command that prints a line and fails with a known code.
fn approved_failure(kind: Kind) -> &'static str {
    match kind {
        Kind::Bash | Kind::Zsh => "echo approved-out; bash -c 'exit 7'",
        Kind::Cmd => "echo approved-out & cmd /c exit 7",
        Kind::PowerShell => "'approved-out'; cmd /c exit 7",
    }
}

fn paste_then_approved(live: &mut Live) {
    let shell = live.shell.clone();
    let from = live.event_count();
    live.terminal.write(&live.id, pasted_block(live.kind)).expect("paste");
    live.wait_state(from, "userRunning");
    // The first command's output arrives; its prompt has been drawn, and the
    // second command is running for another few seconds.
    assert!(
        wait_until(TIMEOUT, || lines_since(&live.sink, from).iter().any(|l| l == "pasted-A")),
        "{shell}: the first pasted command never printed; {:?}",
        lines_since(&live.sink, from)
    );
    std::thread::sleep(Duration::from_millis(300));
    let (_, refused) = live.try_start("echo should-not-run");
    assert_eq!(
        refused.expect_err("execute accepted while the second pasted command runs"),
        USER_RUNNING_ERROR,
        "{shell}: the first prompt of a two-line paste returned the session to Ready"
    );
    // Ready comes only with the last prompt, after the second command's output.
    live.wait_state(from, "ready");
    let lines = lines_since(&live.sink, from);
    assert!(lines.iter().any(|l| l == "pasted-B"), "{shell}: Ready before the second pasted command finished: {lines:?}");
    std::thread::sleep(Duration::from_millis(500));
    let before = live.event_count();
    let command = approved_failure(live.kind);
    let (exit, approved) = live.run(command);
    assert_eq!(exit, 7, "{shell}: the approved command's own exit code after a paste; {approved:?}");
    assert!(approved.iter().any(|l| l == "approved-out"), "{shell}: {approved:?}");
    assert!(!approved.iter().any(|l| l == "should-not-run" || l == "pasted-B"), "{shell}: {approved:?}");
    // Order, over the whole interaction.
    let all = lines_since(&live.sink, from);
    assert!(
        contains_in_order(&all, &["pasted-A".to_string(), "pasted-B".to_string(), "approved-out".to_string()]),
        "{shell}: output out of order: {all:?}"
    );
    assert!(!all.iter().any(|l| l == "should-not-run"), "{shell}: a refused command ran: {all:?}");
    let _ = before;
}

/// Up to 200 characters either side of the first `needle` in `text`.
fn around(text: &str, needle: &str) -> String {
    let Some(at) = text.find(needle) else { return String::new() };
    let start = text[..at].char_indices().rev().nth(199).map(|(i, _)| i).unwrap_or(0);
    let end = text[at..].char_indices().nth(200 + needle.len()).map(|(i, _)| at + i).unwrap_or(text.len());
    text[start..end].to_string()
}

/// A long working directory, the console at 80 and then 120 columns, and more
/// commands than a screen holds: the marker is not a row of text, so the width
/// of the console, wraps, and the first screen fill have no bearing on whether
/// a command finishes (F-a13c07e1, F-86921478, F-84f2740d).
fn exercise_long_cwd(shell: &str, kind: Kind, tag: &str) {
    let dir = long_work_dir(tag);
    let dir_len = dir.to_string_lossy().len();
    let mut live = start(shell, kind, &dir);
    assert!(
        live.last_cwd().len() + 40 >= dir_len,
        "{shell}: ready cwd {:?} is not the long directory ({dir_len} characters)",
        live.last_cwd()
    );
    // A resize makes ConPTY repaint the screen, boot echo included: start from
    // a clean screen so what is checked below is what the commands showed.
    let clear = match kind {
        Kind::Cmd => "cls",
        Kind::PowerShell => "Clear-Host",
        Kind::Bash | Kind::Zsh => "clear",
    };
    let (exit, _) = live.run(clear);
    assert_eq!(exit, 0, "{shell}: clear the screen");
    std::thread::sleep(Duration::from_millis(500));
    live.ready_at = live.event_count();
    for cols in [80u16, 120] {
        live.terminal.resize(&live.id, cols, 30).expect("resize");
        std::thread::sleep(Duration::from_millis(500));
        for n in 1..=38 {
            let word = format!("w{cols}n{n}");
            let (exit, lines) = live.run_fast(&format!("echo {word}"));
            assert_eq!(exit, 0, "{shell} at {cols} columns: command {n}; lines {lines:?}");
            assert!(
                lines.iter().any(|l| *l == word),
                "{shell} at {cols} columns: command {n} lost its output; {lines:?}"
            );
        }
        let (exit, lines) = live.run_fast(exit_seven(kind));
        assert_eq!(exit, 7, "{shell} at {cols} columns: exact exit code; {lines:?}");
        assert!(
            live.last_cwd().len() + 40 >= dir_len,
            "{shell} at {cols} columns: cwd {:?} lost its length",
            live.last_cwd()
        );
    }
    // The finish was reported once for every command.
    std::thread::sleep(Duration::from_millis(500));
    let finishes = live
        .sink
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, RuntimeEvent::ExecutionFinished(_)))
        .count();
    assert_eq!(finishes, 2 * 39 + 1, "{shell}: every command finished exactly once");
    let shown = live.shown_since_ready();
    for needle in ["7733", "COMMANDUI", "__cu"] {
        assert!(!shown.contains(needle), "{shell}: `{needle}` was displayed; ready_at {}; around it: {:?}; events:
{}", live.ready_at, around(&shown, needle), live.debug_events(live.ready_at + 6));
    }
    drop(live);
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("commandui-long-{}-{tag}", std::process::id())));
}

/// A working directory whose name is double-width characters, wider than one
/// console row in cells (and not in characters): the cwd travels in a marker
/// as percent-escaped UTF-8 and must come back whole (F-84f2740d).
fn exercise_wide_cwd(shell: &str, kind: Kind, tag: &str) {
    let root = std::env::temp_dir().join(format!("commandui-wide-{}-{tag}", std::process::id()));
    let mut dir = root.clone();
    for n in 0..3 {
        dir = dir.join(format!("{n}-{}", "\u{65e5}\u{672c}\u{8a9e}".repeat(8)));
    }
    std::fs::create_dir_all(&dir).unwrap();
    let cells: usize = dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii() { 1 } else { 2 })
        .sum();
    assert!(cells > 40);
    let mut live = start(shell, kind, &dir);
    let wanted = last_component(&dir.to_string_lossy());
    assert_eq!(last_component(&live.last_cwd()), wanted, "{shell}: the wide cwd came back whole");
    for n in 1..=6 {
        let word = format!("wide{n}");
        let (exit, lines) = live.run_fast(&format!("echo {word}"));
        assert_eq!(exit, 0, "{shell}: command {n} in a wide cwd; {lines:?}");
        assert!(lines.iter().any(|l| *l == word), "{shell}: {lines:?}");
    }
    assert_eq!(last_component(&live.last_cwd()), wanted, "{shell}: the wide cwd after the commands");
    let shown = live.shown_since_ready();
    assert!(!shown.contains("7733"), "{shell}: marker displayed");
    drop(live);
    let _ = std::fs::remove_dir_all(root);
}

/// The whole contract, against one real shell.
fn exercise(shell: &str, kind: Kind, tag: &str) {
    let dir = work_dir(tag);
    let mut live = start(shell, kind, &dir);

    // Ready, and the cwd it reported is the one we asked for.
    let folder = last_component(&dir.to_string_lossy());
    assert_eq!(last_component(&live.last_cwd()), folder, "{shell}: ready cwd");

    // A command finishes with exit 0 and a bare output line, and the session's
    // marker plumbing is not shown.
    let (exit, lines) = live.run("echo probe-ok");
    assert_eq!(exit, 0, "{shell}: echo exit; lines {lines:?}");
    assert!(lines.iter().any(|l| l == "probe-ok"), "{shell}: no bare probe-ok line in {lines:?}");
    let shown = lines.join("\n");
    assert!(!shown.contains("COMMANDUI"), "{shell}: marker plumbing was displayed: {shown}");
    assert!(!shown.contains("__cu"), "{shell}: marker plumbing was displayed: {shown}");
    assert!(!shown.contains("7733"), "{shell}: marker plumbing was displayed: {shown}");

    // Half-typed input is discarded, with the cursor at the end of the line...
    live.half_typed_then_run(&["xyz"], "probe-two", "cursor at end");
    // ...and with the cursor in the middle of it (the tail must go too).
    live.half_typed_then_run(&["xyz", "\x1b[D\x1b[D"], "probe-mid", "cursor mid-line");

    // A failing command reports a non-zero exit, and a command with a known
    // code reports exactly that code, once.
    let (exit, lines) = live.run("commandui_no_such_command_zz");
    assert_ne!(exit, 0, "{shell}: failing command reported success; lines {lines:?}");
    let (exit, lines) = live.run(exit_seven(kind));
    assert_eq!(exit, 7, "{shell}: exact exit code; lines {lines:?}");

    // The shell is still usable after a failure, and cwd follows `cd`.
    let (exit, lines) = live.run("cd sub");
    assert_eq!(exit, 0, "{shell}: cd exit; lines {lines:?}");
    assert_eq!(last_component(&live.last_cwd()), "sub", "{shell}: cwd after cd");
    let (exit, _) = live.run("echo probe-three");
    assert_eq!(exit, 0, "{shell}: exit after cd");

    // Output longer than a screen arrives complete, in order, one line each.
    let (exit, lines) = live.run(thirty_lines(kind));
    assert_eq!(exit, 0, "{shell}: multi-line exit; lines {lines:?}");
    let wanted: Vec<String> = (1..=30).map(|n| format!("line{n}")).collect();
    assert!(
        contains_in_order(&lines, &wanted),
        "{shell}: 30 lines not complete and in order: {lines:?}\nraw: {:?}",
        raw_text(&live.sink)
    );
    for n in 1..=30 {
        let want = format!("line{n}");
        assert_eq!(lines.iter().filter(|l| **l == want).count(), 1, "{shell}: {want} not shown once: {lines:?}");
    }

    // ESC followed by non-ASCII output used to panic the reader thread.
    let (exit, _) = live.run(escape_then_non_ascii(kind));
    assert_eq!(exit, 0, "{shell}: ESC + non-ASCII output");
    let (exit, lines) = live.run("echo after-esc");
    assert_eq!(exit, 0, "{shell}: exit after ESC + non-ASCII");
    assert!(lines.iter().any(|l| l == "after-esc"), "{shell}: {lines:?}");

    // An alternate screen that is entered and never left must not swallow the
    // next prompt marker.
    let (exit, _) = live.run(enter_alt_screen_forever(kind));
    assert_eq!(exit, 0, "{shell}: entering the alternate screen");
    let (exit, lines) = live.run("echo after-alt");
    assert_eq!(exit, 0, "{shell}: exit after the alternate screen was left open");
    assert!(lines.iter().any(|l| l == "after-alt"), "{shell}: marker or output lost after the alternate screen: {lines:?}");
    let (exit, _) = live.run("echo after-alt-two");
    assert_eq!(exit, 0, "{shell}: second command after the alternate screen");

    // cmd specifics: `!` in a command, a trailing rem, a program that flushes
    // the console input buffer, lines that cannot carry the exit-code tail.
    if kind == Kind::Cmd {
        let (exit, lines) = live.run("echo hello!");
        assert_eq!(exit, 0, "{shell}: bang exit; {lines:?}");
        assert!(lines.iter().any(|l| l == "hello!"), "{shell}: `!` was eaten: {lines:?}");
        let (exit, lines) = live.run("echo rem-ok & rem");
        assert_eq!(exit, 0, "{shell}: trailing rem; {lines:?}");
        // Two built-ins in a row: neither may report a code the command
        // before it left behind. The exit-code tail ends in `set /p` on empty
        // input, which leaves ERRORLEVEL at 1: only the `(call )` reset in
        // front of every approved line makes the next echo report 0.
        let (exit, lines) = live.run("echo first-builtin");
        assert_eq!(exit, 0, "{shell}: first built-in; {lines:?}");
        let (exit, lines) = live.run("echo second-builtin");
        assert_eq!(exit, 0, "{shell}: a built-in after another command reported its leftover code; {lines:?}");
        let (exit, _) = live.run("cd .");
        assert_eq!(exit, 0, "{shell}: cd after a command");
        let (exit, _) = live.run("cd ..");
        assert_eq!(exit, 0, "{shell}: cd .. after a command");
        // `if` and `for` take the rest of the line, and a trailing operator
        // does not parse with the tail after it (F-108e81bd): none of them may
        // leave the session Running, and the exit codes they have are kept.
        let (exit, lines) = live.run("if 1==1 cmd /c exit 3");
        assert_eq!(exit, 3, "{shell}: if keeps the exit code; {lines:?}");
        let (exit, lines) = live.run("echo end-amp &");
        assert_eq!(exit, 0, "{shell}: trailing &; {lines:?}");
        assert!(lines.iter().any(|l| l == "end-amp"), "{shell}: {lines:?}");
        let (_exit, lines) = live.run("echo end-pipe |");
        assert!(!lines.is_empty(), "{shell}: trailing |; {lines:?}");
        let (exit, lines) = live.run("echo end-redir >");
        assert_ne!(exit, 0, "{shell}: a redirection with no target is an error; {lines:?}");
        let (exit, lines) = live.run("echo end-and &&");
        assert_ne!(exit, 0, "{shell}: a trailing && is an error; {lines:?}");
        let (exit, lines) = live.run("echo for you");
        assert_eq!(exit, 0, "{shell}: `for` as an argument; {lines:?}");
        assert!(lines.iter().any(|l| l == "for you"), "{shell}: {lines:?}");
        let (exit, lines) = live.run("echo after-operators");
        assert_eq!(exit, 0, "{shell}: the session works after the odd lines; {lines:?}");
        assert!(lines.iter().any(|l| l == "after-operators"), "{shell}: {lines:?}");
        let exec = live.start_command("pause");
        assert!(
            wait_until(TIMEOUT, || output_lines(&live.sink).iter().any(|l| l.contains("Press any key"))),
            "{shell}: pause never asked for a key: {:?}\nraw: {:?}",
            output_lines(&live.sink),
            raw_text(&live.sink)
        );
        live.terminal.write(&live.id, " ").expect("press a key");
        let finished = live.finish(&exec, "pause");
        assert_eq!(finished.exit, 0, "{shell}: pause");
        let (exit, _) = live.run("echo after-pause");
        assert_eq!(exit, 0, "{shell}: command after pause");
    }

    // A command the runtime ran can be interrupted, and the session recovers.
    let long = live.long_command();
    let exec = live.start_command(long);
    std::thread::sleep(Duration::from_millis(1500));
    live.terminal.interrupt(&live.id).expect("interrupt");
    let finished = live.finish(&exec, long);
    assert_eq!(finished.status, "interrupted", "{shell}: interrupted command status");
    let (exit, _) = live.run("echo after-interrupt");
    assert_eq!(exit, 0, "{shell}: command after interrupt");

    // resync on an idle session returns to ready and the shell still works.
    let from = live.event_count();
    live.terminal.resync(&live.id).expect("resync");
    live.wait_state(from, "ready");
    std::thread::sleep(Duration::from_millis(500));
    let (exit, _) = live.run("echo after-resync");
    assert_eq!(exit, 0, "{shell}: command after resync");

    // A command the user typed by hand: the session is not ready until the
    // prompt returns, and execute says why.
    let from = live.event_count();
    live.terminal
        .write(&live.id, &format!("{}\r", live.long_command().replace("20", "6")))
        .expect("type a command");
    live.wait_state(from, "userRunning");
    let (_, refused) = live.try_start("echo should-not-run");
    assert_eq!(refused.unwrap_err(), USER_RUNNING_ERROR, "{shell}: execute during a typed command");
    live.wait_state(from, "ready");
    std::thread::sleep(Duration::from_millis(500));
    let (exit, lines) = live.run("echo after-typed");
    assert_eq!(exit, 0, "{shell}: execute after the typed command ended");
    assert!(!lines.join("\n").contains("should-not-run"), "{shell}: refused command ran: {lines:?}");
    // ...and it can be interrupted.
    let from = live.event_count();
    live.terminal.write(&live.id, &format!("{}\r", live.long_command())).expect("type a command");
    live.wait_state(from, "userRunning");
    std::thread::sleep(Duration::from_millis(1000));
    live.terminal.interrupt(&live.id).expect("interrupt a typed command");
    live.wait_state(from, "ready");
    std::thread::sleep(Duration::from_millis(500));
    let (exit, _) = live.run("echo after-typed-interrupt");
    assert_eq!(exit, 0, "{shell}: execute after interrupting a typed command");

    // A built-in that does not touch ERRORLEVEL (cmd's echo) must not report
    // the code the command before it left behind. (Every approved cmd line ends
    // in a tail whose `set /p` on empty input leaves ERRORLEVEL at 1, so the
    // `(call )` reset in front of the line is what makes the consecutive
    // built-ins above, and the ones here, report 0. Verified on a scratch copy,
    // 2026-10-03: with the reset removed from command_line_for_shell, live_cmd
    // is red at the first echo after another approved command.)
    let (exit, _) = live.run(exit_seven(kind));
    assert_eq!(exit, 7, "{shell}: a failing command before the built-in");
    let (exit, lines) = live.run("echo after-failure");
    assert_eq!(exit, 0, "{shell}: a built-in after a failure reported the old code; {lines:?}");
    let from = live.event_count();
    live.terminal.write(&live.id, &format!("{}\r", live.long_command())).expect("type a command");
    live.wait_state(from, "userRunning");
    std::thread::sleep(Duration::from_millis(1000));
    live.terminal.interrupt(&live.id).expect("interrupt a typed command");
    live.wait_state(from, "ready");
    std::thread::sleep(Duration::from_millis(500));
    let (exit, lines) = live.run("echo after-second-interrupt");
    assert_eq!(exit, 0, "{shell}: a built-in after Ctrl+C reported the interrupt code; {lines:?}");

    // A pasted block of two lines: the first prompt must not return the
    // session to Ready while the second command still runs, and an approved
    // command written afterwards finishes on ITS OWN prompt with ITS exit code,
    // after both pasted commands (F-8792debb, F-d4a95b36).
    paste_then_approved(&mut live);

    // Editing modes: the approved command must replace whatever is typed,
    // whichever mode the line editor is in.
    match kind {
        Kind::PowerShell => {
            for mode in ["Emacs", "Vi", "Windows"] {
                let (exit, lines) = live.run(&format!("Set-PSReadLineOption -EditMode {mode}"));
                assert_eq!(exit, 0, "{shell}: switch to {mode}; {lines:?}");
                std::thread::sleep(Duration::from_millis(300));
                live.half_typed_then_run(&["echo Xhalf"], "probe-insert", &format!("{mode}, typing"));
                if mode == "Vi" {
                    // Escape puts PSReadLine's vi editor in normal mode, where
                    // letters are commands, not text.
                    live.half_typed_then_run(&["echo Xhalf", "\x1b"], "probe-normal", "vi normal mode");
                    live.half_typed_then_run(&["echo Xhalf", "\x1b", "0"], "probe-normal-two", "vi normal mode, cursor moved");
                }
            }
        }
        Kind::Bash => {
            let (exit, lines) = live.run("set -o vi");
            assert_eq!(exit, 0, "{shell}: set -o vi; {lines:?}");
            std::thread::sleep(Duration::from_millis(300));
            live.half_typed_then_run(&["echo Xhalf"], "probe-insert", "vi insert mode");
            live.half_typed_then_run(&["echo Xhalf", "\x1b"], "probe-normal", "vi normal mode");
            let (exit, lines) = live.run("set -o emacs");
            assert_eq!(exit, 0, "{shell}: set -o emacs; {lines:?}");
            std::thread::sleep(Duration::from_millis(300));
            live.half_typed_then_run(&["echo Xhalf"], "probe-emacs", "emacs mode");
        }
        Kind::Cmd | Kind::Zsh => {}
    }

    // Nothing of the marker machinery was ever shown after the session became
    // ready: no sequence, no plumbing, in any display line.
    let shown = live.shown_since_ready();
    for needle in ["7733", "COMMANDUI", "__cu", "ERRORLEVEL"] {
        assert!(!shown.contains(needle), "{shell}: `{needle}` was displayed; ready_at {}; around it: {:?}; events:
{}", live.ready_at, around(&shown, needle), live.debug_events(live.ready_at + 6));
    }
    assert!(!shown.contains("\x1b]7733"), "{shell}: a marker sequence reached the display");
    for line in output_lines(&live.sink).iter().skip(1) {
        assert!(!line.contains("__COMMANDUI"), "{shell}: marker text in a display line: {line:?}");
    }

    // The shell exits: the watcher notices (ConPTY never closes its pipe), the
    // session reports it, and nothing more can be run in it.
    let from = live.event_count();
    live.terminal.write(&live.id, "exit\r").expect("type exit");
    live.wait_state(from, "desynced");
    let exited = wait_until(TIMEOUT, || {
        live.sessions
            .list()
            .unwrap()
            .iter()
            .any(|s| s.id == live.id && s.status == "exited")
    });
    assert!(exited, "{shell}: session never reported exited");
    let (_, refused) = live.try_start("echo nope");
    assert!(refused.unwrap_err().contains("has exited"), "{shell}: execute on an exited session");

    drop(live);
    let _ = std::fs::remove_dir_all(dir.ancestors().nth(2).unwrap_or(&dir));
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

/// Is `key` required by a `COMMANDUI_REQUIRE_SHELLS` value? `1` (or `all`)
/// requires every shell; a comma-separated list requires the named ones.
fn required_in(value: &str, key: &str) -> bool {
    let value = value.trim().to_lowercase();
    value == "1" || value == "all" || value.split(',').any(|k| k.trim() == key)
}

/// A shell that is not here is skipped with a message, or fails the test when
/// the environment says it must be present.
fn skip(key: &str, shell: &str, why: &str) {
    let required = std::env::var("COMMANDUI_REQUIRE_SHELLS")
        .map(|v| required_in(&v, key))
        .unwrap_or(false);
    skip_if(required, shell, why);
}

fn skip_if(required: bool, shell: &str, why: &str) {
    if required {
        panic!("live shell test for {shell} cannot run ({why}) and COMMANDUI_REQUIRE_SHELLS requires it");
    }
    eprintln!("SKIPPED live shell test for {shell}: {why}");
}

#[cfg(windows)]
#[test]
fn live_cmd() {
    exercise("cmd.exe", Kind::Cmd, "cmd");
}

#[cfg(windows)]
#[test]
fn live_windows_powershell() {
    exercise("powershell.exe", Kind::PowerShell, "ps5");
}

#[cfg(windows)]
#[test]
fn live_cmd_wide_cwd() {
    exercise_wide_cwd("cmd.exe", Kind::Cmd, "cmdwide");
}

#[cfg(windows)]
#[test]
fn live_windows_powershell_wide_cwd() {
    exercise_wide_cwd("powershell.exe", Kind::PowerShell, "ps5wide");
}

#[cfg(windows)]
#[test]
fn live_pwsh_wide_cwd() {
    match on_path("pwsh.exe") {
        Some(path) => exercise_wide_cwd(&path, Kind::PowerShell, "pwshwide"),
        None => skip("pwsh", "pwsh.exe", "not on PATH"),
    }
}

#[cfg(windows)]
#[test]
fn live_git_bash_wide_cwd() {
    match first_existing(&[
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ]) {
        Some(path) => exercise_wide_cwd(&path, Kind::Bash, "gitbashwide"),
        None => skip("gitbash", "Git Bash", "not installed"),
    }
}

#[cfg(windows)]
#[test]
fn live_cmd_long_cwd() {
    exercise_long_cwd("cmd.exe", Kind::Cmd, "cmdlong");
}

#[cfg(windows)]
#[test]
fn live_windows_powershell_long_cwd() {
    exercise_long_cwd("powershell.exe", Kind::PowerShell, "ps5long");
}

#[cfg(windows)]
#[test]
fn live_pwsh() {
    match on_path("pwsh.exe") {
        Some(path) => exercise(&path, Kind::PowerShell, "pwsh"),
        None => skip("pwsh", "pwsh.exe", "not on PATH"),
    }
}

#[cfg(windows)]
#[test]
fn live_pwsh_long_cwd() {
    match on_path("pwsh.exe") {
        Some(path) => exercise_long_cwd(&path, Kind::PowerShell, "pwshlong"),
        None => skip("pwsh", "pwsh.exe", "not on PATH"),
    }
}

#[cfg(windows)]
#[test]
fn live_git_bash_on_windows() {
    match first_existing(&[
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ]) {
        Some(path) => exercise(&path, Kind::Bash, "gitbash"),
        None => skip("gitbash", "Git Bash", "not installed"),
    }
}

#[cfg(windows)]
#[test]
fn live_git_bash_long_cwd() {
    match first_existing(&[
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ]) {
        Some(path) => exercise_long_cwd(&path, Kind::Bash, "gitbashlong"),
        None => skip("gitbash", "Git Bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_bash() {
    // COMMANDUI_LIVE_BASH points the test at another bash build (CI runs the
    // distribution's; a developer can check an older readline).
    let chosen = std::env::var("COMMANDUI_LIVE_BASH").ok();
    let mut candidates: Vec<&str> = chosen.iter().map(|s| s.as_str()).collect();
    candidates.extend(["/bin/bash", "/usr/bin/bash"]);
    match first_existing(&candidates) {
        Some(path) => exercise(&path, Kind::Bash, "bash"),
        None => skip("bash", "bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_bash_wide_cwd() {
    let chosen = std::env::var("COMMANDUI_LIVE_BASH").ok();
    let mut candidates: Vec<&str> = chosen.iter().map(|s| s.as_str()).collect();
    candidates.extend(["/bin/bash", "/usr/bin/bash"]);
    match first_existing(&candidates) {
        Some(path) => exercise_wide_cwd(&path, Kind::Bash, "bashwide"),
        None => skip("bash", "bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_bash_long_cwd() {
    let chosen = std::env::var("COMMANDUI_LIVE_BASH").ok();
    let mut candidates: Vec<&str> = chosen.iter().map(|s| s.as_str()).collect();
    candidates.extend(["/bin/bash", "/usr/bin/bash"]);
    match first_existing(&candidates) {
        Some(path) => exercise_long_cwd(&path, Kind::Bash, "bashlong"),
        None => skip("bash", "bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_zsh() {
    match first_existing(&["/bin/zsh", "/usr/bin/zsh", "/opt/homebrew/bin/zsh"]) {
        Some(path) => exercise(&path, Kind::Zsh, "zsh"),
        None => skip("zsh", "zsh", "not installed"),
    }
}

#[test]
fn visible_lines_follows_a_terminal() {
    // bash redraws a line it cleared as backspaces, spaces, backspaces.
    assert_eq!(
        visible_lines("$ echo Xhalf\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}          \u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}echo ok\r\nok\r\n"),
        vec!["$ echo ok", "ok", ""]
    );
    assert_eq!(visible_lines("a\r\nb\rc\r\n"), vec!["a", "c", ""]);
    assert_eq!(
        visible_lines("\u{1b}[?25lone\u{1b}[5;1Htwo\u{1b}]0;title\u{7}\r\n"),
        vec!["one", "two", ""]
    );
}

#[test]
fn visible_lines_keeps_what_an_overwrite_does_not_reach() {
    // A tail-preserving overwrite: the surviving tail is on screen.
    assert_eq!(visible_lines("abc\u{8}\u{8}X"), vec!["aXc"]);
    // CR then shorter text: the rest of the row stays.
    assert_eq!(visible_lines("echo xyz\rabcd"), vec!["abcd xyz"]);
    assert_eq!(visible_lines("echo xyz\r\n"), vec!["echo xyz", ""]);
    // Backspace at column 0 goes nowhere.
    assert_eq!(visible_lines("\u{8}\u{8}ab"), vec!["ab"]);
    assert_eq!(visible_lines("x\r\u{8}y"), vec!["y"]);
    // Erase in line, delete character, erase character.
    assert_eq!(visible_lines("hello world\r\u{1b}[5C\u{1b}[K"), vec!["hello"]);
    assert_eq!(visible_lines("hello\u{8}\u{8}\u{8}\u{1b}[K"), vec!["he"]);
    assert_eq!(visible_lines("abcdef\r\u{1b}[2P"), vec!["cdef"]);
    assert_eq!(visible_lines("abcdef\r\u{1b}[3X"), vec!["def"]);
    assert_eq!(visible_lines("abcdef\u{1b}[2K"), vec![""]);
    // A short overwrite that leaves part of a typed line behind is visible.
    assert_eq!(visible_lines("> echo xhalf\r> echo\u{1b}[K\r\n"), vec!["> echo", ""]);
    assert_eq!(visible_lines("> echo xhalf\r> echo ok\r\n"), vec!["> echo okalf", ""]);
}

#[test]
fn skipped_shells_fail_when_they_are_required() {
    skip_if(false, "nonexistent-shell", "this is a probe of skip_if()");
    let required = std::panic::catch_unwind(|| {
        skip_if(true, "nonexistent-shell", "this is a probe of skip_if()");
    });
    assert!(required.is_err(), "a skipped shell did not fail when it was required");
}

#[test]
fn require_shells_takes_all_or_a_list() {
    assert!(required_in("1", "bash") && required_in("1", "zsh"));
    assert!(required_in("all", "cmd"));
    assert!(required_in("bash", "bash"));
    assert!(!required_in("bash", "zsh"), "a job that has bash must not be made to have zsh");
    assert!(required_in("bash, cmd ,pwsh", "cmd") && required_in("Bash,CMD", "cmd"));
    assert!(!required_in("", "bash") && !required_in("0", "bash"));
}

#[cfg(windows)]
#[test]
fn default_shell_prefers_pwsh_on_path_over_windows_powershell() {
    if std::env::var_os("COMMANDUI_WINDOWS_SHELL").is_some() {
        return skip("default_shell", "default_shell", "COMMANDUI_WINDOWS_SHELL overrides it");
    }
    let Some(on_path_pwsh) = on_path("pwsh.exe") else {
        return skip("default_shell", "default_shell", "pwsh.exe is not on PATH, so 5.1 is the right answer");
    };
    let chosen = commandui_runtime_core::pty::default_shell();
    assert!(chosen.to_lowercase().ends_with("pwsh.exe"), "chose {chosen}, pwsh is at {on_path_pwsh}");
}
