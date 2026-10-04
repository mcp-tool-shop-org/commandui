//! Live shell tests: drive real shells through SessionService and
//! TerminalService. Every unit test in the crate asserts bytes; these prove
//! that a real shell actually becomes ready, runs a command, discards
//! half-typed input in every editing mode, reports its exact exit code,
//! survives hostile output, can be interrupted and resynced, reports that a
//! command the user typed is running, and reports when it exits.
//!
//! A shell that is not installed on this machine is skipped with a message on
//! stderr, never silently, unless `COMMANDUI_REQUIRE_SHELLS=1` is set (CI):
//! then a missing shell fails the test.
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

/// What a terminal would show: escape sequences dropped, a cursor-position
/// escape starts a new row, a carriage return overwrites the row so far, and a
/// backspace moves the cursor left so that what is written next overwrites
/// (bash redraws a cleared line as backspaces, spaces, backspaces: the text it
/// erased is not on screen).
fn visible_lines(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut rows: Vec<String> = vec![String::new()];
    // After a bare carriage return the cursor is at column 0: what is already
    // on the row stays until something is written over it.
    let mut carriage = false;
    // How many characters the cursor is to the left of the end of the row.
    let mut back = 0usize;
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
                            back = 0;
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
            '\r' => {
                if chars.get(i + 1) != Some(&'\n') {
                    carriage = true;
                }
            }
            '\n' => {
                carriage = false;
                back = 0;
                rows.push(String::new());
            }
            '\u{7}' => {}
            '\u{8}' => {
                back = (back + 1).min(rows.last().unwrap().chars().count());
            }
            _ => {
                if carriage {
                    rows.last_mut().unwrap().clear();
                    carriage = false;
                    back = 0;
                }
                let row = rows.last_mut().unwrap();
                if back > 0 {
                    let mut cells: Vec<char> = row.chars().collect();
                    let at = cells.len() - back;
                    cells[at] = c;
                    *row = cells.into_iter().collect();
                    back -= 1;
                } else {
                    row.push(c);
                }
            }
        }
        i += 1;
    }
    rows.into_iter().map(|r| r.trim().to_string()).collect()
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
    Live {
        sink,
        sessions,
        terminal,
        id,
        n: 0,
        kind,
        shell: shell.to_string(),
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
    assert!(!shown.contains("__cui"), "{shell}: marker plumbing was displayed: {shown}");

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
    // the console input buffer.
    if kind == Kind::Cmd {
        let (exit, lines) = live.run("echo hello!");
        assert_eq!(exit, 0, "{shell}: bang exit; {lines:?}");
        assert!(lines.iter().any(|l| l == "hello!"), "{shell}: `!` was eaten: {lines:?}");
        let (exit, lines) = live.run("echo rem-ok & rem");
        assert_eq!(exit, 0, "{shell}: trailing rem; {lines:?}");
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
    // the code the command before it left behind.
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

/// A shell that is not here is skipped with a message, or fails the test when
/// the environment says every shell must be present.
fn skip(shell: &str, why: &str) {
    let required = std::env::var("COMMANDUI_REQUIRE_SHELLS").map(|v| v == "1").unwrap_or(false);
    skip_if(required, shell, why);
}

fn skip_if(required: bool, shell: &str, why: &str) {
    if required {
        panic!("live shell test for {shell} cannot run ({why}) and COMMANDUI_REQUIRE_SHELLS=1");
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
fn live_pwsh() {
    match on_path("pwsh.exe") {
        Some(path) => exercise(&path, Kind::PowerShell, "pwsh"),
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
        Some(path) => exercise(&path, Kind::Bash, "gitbash"),
        None => skip("Git Bash", "not installed"),
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
        None => skip("bash", "not installed"),
    }
}

#[cfg(unix)]
#[test]
fn live_zsh() {
    match first_existing(&["/bin/zsh", "/usr/bin/zsh", "/opt/homebrew/bin/zsh"]) {
        Some(path) => exercise(&path, Kind::Zsh, "zsh"),
        None => skip("zsh", "not installed"),
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
fn skipped_shells_fail_when_they_are_required() {
    skip_if(false, "nonexistent-shell", "this is a probe of skip_if()");
    let required = std::panic::catch_unwind(|| {
        skip_if(true, "nonexistent-shell", "this is a probe of skip_if()");
    });
    assert!(required.is_err(), "a skipped shell did not fail with COMMANDUI_REQUIRE_SHELLS=1");
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
