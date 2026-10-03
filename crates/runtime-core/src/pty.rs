use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub const PROMPT_MARKER: &str = "__COMMANDUI_PROMPT__";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellFamily {
    PowerShell,
    Cmd,
    Bash,
    Zsh,
    Unsupported,
}

pub(crate) fn shell_family(shell: &str) -> ShellFamily {
    // Match the executable's file stem exactly, never a substring of the path.
    let name = shell.rsplit(['/', '\\']).next().unwrap_or(shell).trim().to_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    match stem {
        "pwsh" | "powershell" => ShellFamily::PowerShell,
        "cmd" => ShellFamily::Cmd,
        "bash" => ShellFamily::Bash,
        "zsh" => ShellFamily::Zsh,
        _ => ShellFamily::Unsupported,
    }
}

pub fn new_marker_nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub type PtyHandle = Arc<Mutex<Box<dyn Write + Send>>>;

pub fn default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(shell) = std::env::var("COMMANDUI_WINDOWS_SHELL") {
            return shell;
        }
        let pwsh7 = format!(
            "{}\\PowerShell\\7\\pwsh.exe",
            std::env::var("ProgramFiles").unwrap_or_default()
        );
        if std::path::Path::new(&pwsh7).exists() {
            return pwsh7;
        }
        "powershell.exe".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

/// The shell's child process handle. Kept so the shell can be killed and reaped.
pub type ShellChild = Box<dyn portable_pty::Child + Send + Sync>;

pub fn spawn_shell(
    shell: &str,
    cwd: Option<&str>,
) -> Result<(PtyPair, PtyHandle, ShellChild), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {e}"))?;

    let cmd = prepare_shell_command(shell, cwd);

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {e}"))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to get PTY writer: {e}"))?;

    let handle: PtyHandle = Arc::new(Mutex::new(writer));

    Ok((pair, handle, child))
}

pub fn write_command(handle: &PtyHandle, command: &str) -> Result<(), String> {
    let mut writer = handle.lock().map_err(|e| format!("Lock error: {e}"))?;
    writer
        .write_all(format!("{command}\n").as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    writer.flush().map_err(|e| format!("Flush error: {e}"))?;
    Ok(())
}

pub fn write_raw(handle: &PtyHandle, data: &str) -> Result<(), String> {
    let mut writer = handle.lock().map_err(|e| format!("Lock error: {e}"))?;
    writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    writer.flush().map_err(|e| format!("Flush error: {e}"))?;
    Ok(())
}

fn prepare_shell_command(shell: &str, cwd: Option<&str>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    // Delayed expansion is deliberately NOT enabled for cmd: it would rewrite
    // every `!` in the user's commands and paths. The marker uses a one-shot
    // `cmd /v:on /c` child instead (see `cmd_marker_echo`).
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    cmd
}

/// The `cmd` marker is printed by a one-shot `cmd /v:on /c` child so delayed
/// expansion never touches the interactive session. `!CD!` is expanded by the
/// child and its value is not re-parsed, so a path containing `& ! ^ |` is safe.
/// The carets are tripled because the parent shell consumes one level.
fn cmd_marker_echo(nonce: &str, exit_expr: &str) -> String {
    format!(
        "cmd /v:on /c echo. ^& echo {PROMPT_MARKER}^^^|{nonce}^^^|!CD!^^^|{exit_expr}"
    )
}

/// Marker line is `\nmarker|nonce|cwd|exit\n`. The nonce is per session.
/// The leading newline guarantees the marker starts its own line even when
/// the previous command left the cursor mid-line.
/// cwd is the shell's full path (`$PWD`, `%CD%`, `Get-Location`), never `~`.
/// cwd is the only field that may contain `|`; `%`, CR and LF in it are
/// written as `%25`, `%0D`, `%0A` and undone by the parser.
///
/// Residual risk: the nonce lives in shell-readable state (PROMPT_COMMAND, the
/// prompt function, the echoed cmd line), so a command running in the session
/// can still print a forged marker and finish itself. The nonce stops stale
/// or unrelated output, not a hostile command in the same shell.
pub fn bootstrap_prompt(shell: &str, nonce: &str) -> Option<String> {
    match shell_family(shell) {
        ShellFamily::PowerShell => Some(format!(
            "function prompt {{ $__cui_ok = $?; $__cui_code = $global:LASTEXITCODE; if ($__cui_ok) {{ $__cui_code = 0 }} elseif (-not ($__cui_code -is [int]) -or $__cui_code -eq 0) {{ $__cui_code = 1 }}; $__cui_cwd = (Get-Location).Path.Replace('%','%25').Replace([string][char]13,'%0D').Replace([string][char]10,'%0A'); $__cui_line = ([string][char]10) + '{PROMPT_MARKER}|{nonce}|' + $__cui_cwd + '|' + $__cui_code; \"$__cui_line`n> \" }}\n"
        )),
        ShellFamily::Cmd => Some(format!("{}\n", cmd_marker_echo(nonce, "%ERRORLEVEL%"))),
        ShellFamily::Bash => Some(format!(
            "__cui_nl=$'\\n'; __cui_cr=$'\\r'; PROMPT_COMMAND='__cui_ec=$?; __cui_cwd=${{PWD//\\%/%25}}; __cui_cwd=${{__cui_cwd//$__cui_nl/%0A}}; __cui_cwd=${{__cui_cwd//$__cui_cr/%0D}}; printf \"\\n{PROMPT_MARKER}|{nonce}|%s|%s\\n\" \"$__cui_cwd\" \"$__cui_ec\"'\n"
        )),
        ShellFamily::Zsh => Some(format!(
            "precmd() {{ local __cui_ec=$? __cui_cwd=\"${{PWD}}\"; __cui_cwd=${{__cui_cwd//\\%/%25}}; __cui_cwd=${{__cui_cwd//$'\\n'/%0A}}; __cui_cwd=${{__cui_cwd//$'\\r'/%0D}}; print -r -- \"\"; print -r -- \"{PROMPT_MARKER}|{nonce}|${{__cui_cwd}}|${{__cui_ec}}\" }}\n"
        )),
        ShellFamily::Unsupported => None,
    }
}

/// Bytes written for one executed command. cmd appends a marker echo because
/// its prompt cannot expand ERRORLEVEL. The command text is the command, not
/// a format string.
pub(crate) fn command_line_for_shell(shell: &str, nonce: &str, command: &str) -> String {
    if shell_family(shell) == ShellFamily::Cmd {
        // The marker is a separate input line so a trailing rem, :: or an
        // unbalanced quote in the command cannot swallow it.
        // The exit code is captured into a variable first (`call` expands
        // `%^ERRORLEVEL%` after the command ran), then the child prints it.
        format!(
            "{command}\ncall set __cui_ec=%^ERRORLEVEL% & {}\n",
            cmd_marker_echo(nonce, "!__cui_ec!")
        )
    } else {
        format!("{command}\n")
    }
}

pub(crate) fn resync_input(shell: &str, nonce: &str) -> String {
    if shell_family(shell) == ShellFamily::Cmd {
        format!("{}\n", cmd_marker_echo(nonce, "%ERRORLEVEL%"))
    } else {
        "\n".to_string()
    }
}

pub(crate) fn clone_reader(pair: &PtyPair) -> Result<Box<dyn Read + Send>, String> {
    pair.master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {e}"))
}

pub(crate) fn spawn_reader<R, F>(reader: R, on_chunk: F)
where
    R: Read + Send + 'static,
    F: Fn(String) + Send + 'static,
{
    spawn_reader_with_exit(reader, on_chunk, || {});
}

/// Like spawn_reader, and calls on_exit once the stream ends (EOF or a read
/// error), which for a PTY means the shell is gone.
pub(crate) fn spawn_reader_with_exit<R, F, E>(mut reader: R, on_chunk: F, on_exit: E)
where
    R: Read + Send + 'static,
    F: Fn(String) + Send + 'static,
    E: FnOnce() + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        // Bytes of a multibyte character that a read cut in half.
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = decode_utf8_stream(&mut pending, &buf[..n]);
                    if !text.is_empty() {
                        on_chunk(text);
                    }
                }
            }
        }
        if !pending.is_empty() {
            on_chunk(String::from_utf8_lossy(&pending).to_string());
        }
        on_exit();
    });
}

/// Decode `incoming` after any bytes held in `pending`. Complete sequences are
/// returned; an incomplete trailing sequence stays in `pending` for the next
/// read; genuinely invalid bytes become U+FFFD.
pub(crate) fn decode_utf8_stream(pending: &mut Vec<u8>, incoming: &[u8]) -> String {
    pending.extend_from_slice(incoming);
    let mut out = String::new();
    let mut start = 0;
    loop {
        match std::str::from_utf8(&pending[start..]) {
            Ok(valid) => {
                out.push_str(valid);
                start = pending.len();
                break;
            }
            Err(err) => {
                let valid_end = start + err.valid_up_to();
                out.push_str(&String::from_utf8_lossy(&pending[start..valid_end]));
                start = valid_end;
                match err.error_len() {
                    Some(bad) => {
                        out.push('\u{FFFD}');
                        start += bad;
                    }
                    None => break,
                }
            }
        }
    }
    pending.drain(..start);
    out
}

pub fn spawn_reader_loop<F>(pair: &PtyPair, on_chunk: F)
where
    F: Fn(String) + Send + 'static,
{
    let reader = pair
        .master
        .try_clone_reader()
        .expect("Failed to clone PTY reader");
    spawn_reader(reader, on_chunk);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    const NONCE: &str = "abc123nonce";

    #[test]
    fn shell_family_matches_the_executable_name_only() {
        assert!(shell_family("/home/cmdr/.local/bin/fish") == ShellFamily::Unsupported);
        assert!(shell_family(r"/opt/zshkit/bin/fish") == ShellFamily::Unsupported);
        assert!(shell_family(r"C:\Tools\bash-utils\nu.exe") == ShellFamily::Unsupported);
        assert!(shell_family(r"C:\Windows\System32\cmd.exe") == ShellFamily::Cmd);
        assert!(shell_family(r"C:\Program Files\PowerShell\7\pwsh.exe") == ShellFamily::PowerShell);
        assert!(shell_family("powershell.exe") == ShellFamily::PowerShell);
        assert!(shell_family("/usr/bin/zsh") == ShellFamily::Zsh);
        assert!(shell_family("bash") == ShellFamily::Bash);
    }

    #[test]
    fn supported_prompts_carry_nonce_exit_and_full_cwd() {
        let pwsh = bootstrap_prompt("powershell.exe", NONCE).unwrap();
        assert!(pwsh.contains(PROMPT_MARKER));
        assert!(pwsh.contains(NONCE));
        assert!(pwsh.contains("$?"));
        assert!(pwsh.contains("LASTEXITCODE"));
        assert!(pwsh.contains("Get-Location"));
        assert!(pwsh.contains("$__cui_code = 1"));
        assert!(pwsh.contains("if ($__cui_ok) { $__cui_code = 0 }"));
        assert!(!pwsh.contains('~'));

        let cmd = bootstrap_prompt("cmd.exe", NONCE).unwrap();
        assert!(cmd.contains("%ERRORLEVEL%"));
        assert!(cmd.contains("!CD!"));
        assert!(cmd.contains(NONCE));
        assert!(cmd.contains("^|"));
        assert!(!cmd.contains("$P"));

        let bash = bootstrap_prompt("/bin/bash", NONCE).unwrap();
        assert!(bash.contains("PWD"));
        assert!(bash.contains("$?"));
        assert!(bash.contains(NONCE));
        assert!(!bash.contains("\\w"));

        let zsh = bootstrap_prompt("/bin/zsh", NONCE).unwrap();
        assert!(zsh.contains("${PWD}"));
        assert!(zsh.contains("$?"));
        assert!(zsh.contains(NONCE));
        assert!(!zsh.contains("%~"));
    }

    #[test]
    fn unsupported_shells_have_no_bootstrap() {
        assert!(bootstrap_prompt("/bin/sh", NONCE).is_none());
        assert!(bootstrap_prompt("dash", NONCE).is_none());
        assert!(bootstrap_prompt("fish", NONCE).is_none());
    }

    #[test]
    fn cmd_launch_does_not_enable_delayed_expansion() {
        let cmd = prepare_shell_command("C:\\Windows\\System32\\cmd.exe", Some("/work"));
        assert!(!cmd.get_argv().iter().any(|arg| arg == "/v:on"));
        assert_eq!(cmd.get_cwd().map(|d| d.to_string_lossy().to_string()).as_deref(), Some("/work"));

        let bash = prepare_shell_command("/bin/bash", None);
        assert!(!bash.get_argv().iter().any(|arg| arg == "/v:on"));
    }

    #[test]
    fn cmd_command_line_appends_marker_without_using_user_text_as_format() {
        let nasty = "echo %CD% & del /q *";
        let line = command_line_for_shell("cmd.exe", NONCE, nasty);
        assert!(line.starts_with(&format!("{nasty}\ncall set ")));
        assert_eq!(line.matches('\n').count(), 2);
        assert!(line.contains(&format!("{PROMPT_MARKER}^^^|{NONCE}^^^|!CD!^^^|!__cui_ec!")));
        assert_eq!(command_line_for_shell("bash", NONCE, "ls"), "ls\n");
        assert_eq!(resync_input("bash", NONCE), "\n");
        assert!(resync_input("cmd.exe", NONCE).contains("%ERRORLEVEL%"));
    }

    #[test]
    fn cmd_commands_with_bangs_are_written_through_unchanged() {
        for command in ["echo hello!", "git commit -m \"done!\"", "cd hello!world"] {
            let line = command_line_for_shell("cmd.exe", NONCE, command);
            assert!(line.starts_with(&format!("{command}\ncall set ")), "{line}");
            // Only the marker child expands `!`; the session itself never does.
            assert!(line.contains("cmd /v:on /c"), "{line}");
        }
        let cmd = prepare_shell_command("cmd.exe", None);
        assert!(!cmd.get_argv().iter().any(|arg| arg == "/v:on"));
    }

    #[test]
    fn markers_start_their_own_line() {
        let bash = bootstrap_prompt("/bin/bash", NONCE).unwrap();
        assert!(bash.contains(&format!("\\n{PROMPT_MARKER}")), "{bash}");
        let pwsh = bootstrap_prompt("pwsh", NONCE).unwrap();
        assert!(pwsh.contains("[char]10) + '"), "{pwsh}");
        let zsh = bootstrap_prompt("/bin/zsh", NONCE).unwrap();
        assert!(zsh.contains("print -r -- \"\";"), "{zsh}");
        assert!(bootstrap_prompt("cmd.exe", NONCE).unwrap().contains("echo. ^& echo"));
    }

    #[test]
    fn utf8_split_across_reads_is_reassembled() {
        let text = "caf\u{e9} \u{2603} /h\u{f6}me/\u{1F600}x";
        let bytes = text.as_bytes();
        for split in 1..bytes.len() {
            let mut pending = Vec::new();
            let mut out = decode_utf8_stream(&mut pending, &bytes[..split]);
            out.push_str(&decode_utf8_stream(&mut pending, &bytes[split..]));
            assert_eq!(out, text, "split at {split}");
            assert!(pending.is_empty());
        }
        let mut pending = Vec::new();
        let out = decode_utf8_stream(&mut pending, b"a\xffb");
        assert_eq!(out, "a\u{FFFD}b");
    }

    #[test]
    fn spawn_reader_keeps_a_split_character_inside_a_marker_cwd() {
        struct Chunks(Vec<Vec<u8>>);
        impl Read for Chunks {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                let next = self.0.remove(0);
                buf[..next.len()].copy_from_slice(&next);
                Ok(next.len())
            }
        }
        let line = format!("{PROMPT_MARKER}|{NONCE}|/h\u{f6}me/\u{e9}|0\n").into_bytes();
        let cut = line.iter().position(|b| *b == 0xc3).unwrap() + 1;
        let chunks = vec![line[..cut].to_vec(), line[cut..].to_vec()];
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let tx = Mutex::new(tx);
        spawn_reader(Chunks(chunks), move |text| {
            tx.lock().unwrap().send(text).unwrap();
        });
        let mut all = String::new();
        while let Ok(part) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            all.push_str(&part);
        }
        assert!(!all.contains('\u{FFFD}'), "{all}");
        assert!(all.contains("/h\u{f6}me/\u{e9}|0"), "{all}");
    }

    #[test]
    fn marker_nonce_is_32_hex_and_default_shell_is_nonempty() {
        let nonce = new_marker_nonce();
        assert_eq!(nonce.len(), 32);
        assert!(nonce.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!default_shell().is_empty());
    }

    #[test]
    fn write_command_and_write_raw_succeed_against_sink() {
        let handle: PtyHandle = Arc::new(Mutex::new(
            Box::new(std::io::sink()) as Box<dyn Write + Send>,
        ));
        write_command(&handle, "echo hi").unwrap();
        write_raw(&handle, "xyz").unwrap();
    }

    #[test]
    fn write_paths_report_write_flush_and_lock_errors() {
        let fail_write: PtyHandle = Arc::new(Mutex::new(Box::new(FailWrite) as Box<dyn Write + Send>));
        let write_err = write_command(&fail_write, "echo hi").unwrap_err();
        assert!(write_err.contains("Write error"), "{write_err}");
        let write_err = write_raw(&fail_write, "xyz").unwrap_err();
        assert!(write_err.contains("Write error"), "{write_err}");

        let fail_flush: PtyHandle =
            Arc::new(Mutex::new(Box::new(FailFlush) as Box<dyn Write + Send>));
        let flush_err = write_command(&fail_flush, "echo hi").unwrap_err();
        assert!(flush_err.contains("Flush error"), "{flush_err}");
        let flush_err = write_raw(&fail_flush, "xyz").unwrap_err();
        assert!(flush_err.contains("Flush error"), "{flush_err}");

        let handle: PtyHandle = Arc::new(Mutex::new(
            Box::new(std::io::sink()) as Box<dyn Write + Send>,
        ));
        let cloned = Arc::clone(&handle);
        let joined = std::thread::spawn(move || {
            let _guard = cloned.lock().unwrap();
            panic!("poison pty lock");
        })
        .join();
        assert!(joined.is_err());
        let lock_err = write_command(&handle, "echo hi").unwrap_err();
        assert!(lock_err.contains("Lock error"), "{lock_err}");
        let lock_err = write_raw(&handle, "xyz").unwrap_err();
        assert!(lock_err.contains("Lock error"), "{lock_err}");
    }

    #[test]
    fn spawn_shell_rejects_a_bash_name_that_is_not_a_program() {
        let err = match spawn_shell("bash-not-installed", None) {
            Err(err) => err,
            Ok(_spawned) => panic!("expected missing bash spawn to fail"),
        };
        assert!(
            err.contains("Failed to spawn shell") || err.contains("Failed to open PTY"),
            "{err}"
        );
    }

    #[test]
    fn spawn_shell_echo_reaches_reader_and_clone_reader_is_ok() {
        let cwd = std::env::temp_dir();
        let cwd = cwd.to_string_lossy().to_string();
        let (pair, handle, mut child) = match spawn_shell(&default_shell(), Some(&cwd)) {
            Ok(spawned) => spawned,
            Err(err) => panic!("spawn returned {err}"),
        };

        let cloned = clone_reader(&pair);
        assert!(cloned.is_ok());
        drop(cloned);

        write_raw(&handle, "echo commandui-pty-ok\r\n").expect("echo write");

        let (tx, rx) = std::sync::mpsc::channel();
        spawn_reader_loop(&pair, move |chunk| {
            let _ = tx.send(chunk);
        });

        let got = rx.recv_timeout(std::time::Duration::from_secs(3));
        assert!(got.is_ok(), "expected a reader chunk within 3 seconds");

        // Ask the shell to leave before the pair drops, so the child does not stay up.
        let _ = write_raw(&handle, "exit\r\n");
        let _ = child.kill();
        let _ = child.wait();
        drop(handle);
        drop(pair);
    }

    #[test]
    fn spawn_reader_forwards_chunks_and_stops_on_eof_or_error() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        spawn_reader(
            ScriptedRead {
                steps: vec![Ok(b"abc".to_vec()), Ok(Vec::new())],
                at: 0,
                done: Some(done_tx),
            },
            move |chunk| {
                let _ = tx.send(chunk);
            },
        );
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(), "abc");
        done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("eof reader did not finish");

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        spawn_reader(
            ScriptedRead {
                steps: vec![Err(std::io::Error::other("boom"))],
                at: 0,
                done: Some(done_tx),
            },
            |_| {},
        );
        done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("error reader did not finish");
    }

    struct FailWrite;

    impl Write for FailWrite {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("write failed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct FailFlush;

    impl Write for FailFlush {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("flush failed"))
        }
    }

    struct ScriptedRead {
        steps: Vec<std::io::Result<Vec<u8>>>,
        at: usize,
        done: Option<std::sync::mpsc::Sender<()>>,
    }

    impl Drop for ScriptedRead {
        fn drop(&mut self) {
            if let Some(done) = self.done.take() {
                let _ = done.send(());
            }
        }
    }

    impl Read for ScriptedRead {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.at >= self.steps.len() {
                return Ok(0);
            }
            let step = std::mem::replace(&mut self.steps[self.at], Ok(Vec::new()));
            self.at += 1;
            match step {
                Ok(bytes) if bytes.is_empty() => Ok(0),
                Ok(bytes) => {
                    let n = bytes.len().min(buf.len());
                    buf[..n].copy_from_slice(&bytes[..n]);
                    Ok(n)
                }
                Err(err) => Err(err),
            }
        }
    }
}
