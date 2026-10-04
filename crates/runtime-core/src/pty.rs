//! Shell processes on a pty, and the markers their prompts emit.
//!
//! # Markers are invisible OSC sequences
//!
//! Every shell is bootstrapped with a prompt hook that writes one operating
//! system command to the terminal, the way VS Code's shell integration does:
//!
//! ```text
//! ESC ] 7733 ; <kind> ; <nonce> ; <exit> ; <chord> ; <cwd> ( BEL | ESC \ )
//! ```
//!
//! * `7733` is a vendor number nothing else here uses (VS Code is 633,
//!   FinalTerm 133, iTerm 1337, ConEmu 9, Windows Terminal 9001). A private
//!   number keeps a shell that also has VS Code's integration (inherited
//!   `TERM_PROGRAM=vscode`) from emitting sequences that look like ours.
//! * `kind` is `P` (a prompt was drawn) or `X` (cmd only: the exit code of the
//!   command that just ran, written just before its prompt).
//! * `nonce` is per session. `exit` is an integer or empty (cmd's prompt cannot
//!   expand ERRORLEVEL). `chord` is `1`/`0` when the shell says whether the
//!   clear-line chord is bound, else empty.
//! * `cwd` is last and percent-encoded: `%`, `;`, ESC, BEL, CR and LF never
//!   appear in it raw, so it cannot end the sequence or split a field.
//!
//! An OSC takes no columns and is never wrapped, repainted or split by the
//! console, so the reader recovers a marker from the raw byte stream
//! (`MarkerScanner`) before any line handling, display rewrite or console
//! width is involved, and never displays it. Verified live against ConPTY on
//! cmd, Windows PowerShell 5.1, PowerShell 7 and Git Bash (it arrives
//! byte-for-byte, in order with the text around it).
//!
//! Residual risk: the nonce lives in shell-readable state (the prompt hook, the
//! echoed cmd line), so a command running in the session can print a forged
//! marker and finish itself. The nonce stops stale or unrelated output, not a
//! hostile command in the same shell.
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

/// The key that submits a line to a PTY. A Windows ConPTY submits on CR
/// (Enter); LF is Ctrl+J there and never runs the line. Unix ptys translate
/// CR to LF (ICRNL) and readline/zle accept it. Every line runtime-core builds
/// itself ends in this; `write_raw` (the user's own keystrokes) never adds it.
pub(crate) const ENTER: &str = "\r";

/// Columns and rows of a new pty.
pub(crate) const PTY_COLS: u16 = 120;
const PTY_ROWS: u16 = 30;

/// What every marker starts with.
const OSC_MARKER_OPEN: &str = "\x1b]7733;";

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
            if shell_family(&shell) != ShellFamily::Unsupported {
                return shell;
            }
            eprintln!("[session] COMMANDUI_WINDOWS_SHELL={shell} is not a supported shell; using the default");
        }
        let pwsh7 = format!(
            "{}\\PowerShell\\7\\pwsh.exe",
            std::env::var("ProgramFiles").unwrap_or_default()
        );
        if std::path::Path::new(&pwsh7).exists() {
            return pwsh7;
        }
        // A Store, winget or scoop install of PowerShell 7 is not under
        // Program Files; it is only on PATH. 7 beats the bundled 5.1.
        if let Some(found) = find_on_path("pwsh.exe", std::env::var_os("PATH").as_deref()) {
            return found.to_string_lossy().to_string();
        }
        "powershell.exe".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        match std::env::var("SHELL") {
            Ok(shell) if shell_family(&shell) != ShellFamily::Unsupported => shell,
            Ok(shell) => {
                eprintln!("[session] SHELL={shell} is not a supported shell; using /bin/bash");
                "/bin/bash".to_string()
            }
            Err(_) => "/bin/bash".to_string(),
        }
    }
}

/// First `dir/exe` that exists, for the directories listed in `path_var`.
/// (An App Execution Alias such as the Store `pwsh.exe` is a reparse point
/// that `metadata` follows, so it counts.)
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn find_on_path(exe: &str, path_var: Option<&std::ffi::OsStr>) -> Option<std::path::PathBuf> {
    let path_var = path_var?;
    std::env::split_paths(path_var)
        .map(|dir| dir.join(exe))
        .find(|candidate| std::fs::metadata(candidate).map(|m| m.is_file()).unwrap_or(false))
}

/// The shell's child process handle. Kept so the shell can be killed and reaped.
pub type ShellChild = Box<dyn portable_pty::Child + Send + Sync>;

pub fn spawn_shell(
    shell: &str,
    cwd: Option<&str>,
) -> Result<(PtyPair, PtyHandle, ShellChild), String> {
    spawn_shell_with_args(shell, cwd, &[])
}

/// `spawn_shell`, with arguments for the shell (see `launch_args`).
pub fn spawn_shell_with_args(
    shell: &str,
    cwd: Option<&str>,
    args: &[String],
) -> Result<(PtyPair, PtyHandle, ShellChild), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: PTY_ROWS,
            cols: PTY_COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {e}"))?;

    let cmd = prepare_shell_command(shell, cwd, args);

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

pub fn write_raw(handle: &PtyHandle, data: &str) -> Result<(), String> {
    let mut writer = handle.lock().map_err(|e| format!("Lock error: {e}"))?;
    writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    writer.flush().map_err(|e| format!("Flush error: {e}"))?;
    Ok(())
}

fn prepare_shell_command(shell: &str, cwd: Option<&str>, args: &[String]) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    cmd.args(args);
    // Delayed expansion is deliberately NOT enabled for cmd: it would rewrite
    // every `!` in the user's commands and paths.
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    #[cfg(unix)]
    {
        // A desktop app started from a launcher has no TERM at all (and CI
        // runners have none either): without one `clear`, `less` and `vim`
        // fail or misbehave. The desktop console is xterm.js. A TERM the
        // console inherited from a real terminal is kept.
        if let Some((term, colorterm)) = default_term(cmd.get_env("TERM")) {
            cmd.env("TERM", term);
            if cmd.get_env("COLORTERM").is_none() {
                cmd.env("COLORTERM", colorterm);
            }
        }
        if shell_family(shell) == ShellFamily::Zsh {
            let zdotdir = cmd.get_env("ZDOTDIR").map(std::path::PathBuf::from);
            let home = cmd.get_env("HOME").map(std::path::PathBuf::from);
            if let Some(dir) = zsh_newuser_guard_dir(home.as_deref(), zdotdir.as_deref()) {
                cmd.env("ZDOTDIR", dir);
            }
        }
    }
    cmd
}

/// The `TERM` (and `COLORTERM`) to give a shell whose environment has none, or
/// has `dumb`. None when the inherited TERM is usable.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn default_term(inherited: Option<&std::ffi::OsStr>) -> Option<(&'static str, &'static str)> {
    match inherited.and_then(|t| t.to_str()).map(str::trim) {
        Some(term) if !term.is_empty() && term != "dumb" => None,
        _ => Some(("xterm-256color", "truecolor")),
    }
}

/// zsh runs its new-user wizard (`zsh-newuser-install`) in an interactive
/// shell when none of `.zshenv .zprofile .zshrc .zlogin` exists in `$ZDOTDIR`
/// (or the home directory), and waits there for a key: a first session of a
/// brand-new zsh user would hang at it. The wizard only looks for those files
/// in the directory `$ZDOTDIR` names at startup, so when the user has no
/// startup files and sets no ZDOTDIR the shell is started with a private one
/// holding a `.zshenv` that unsets ZDOTDIR again (so `$ZDOTDIR` is unset in the
/// session, as it would be, and any file the user adds is read as usual) and
/// sets `skip_global_compinit` (Debian and Ubuntu's global zshrc then does not
/// run compinit, which asks a question when a directory on fpath is
/// group-writable, as on CI runners) and removes the directory. The user's own files are never touched or created.
///
/// None when the wizard would not run (a startup file exists, or ZDOTDIR is
/// set), or the directory cannot be made.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn zsh_newuser_guard_dir(
    home: Option<&std::path::Path>,
    zdotdir: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    if zdotdir.is_some_and(|d| !d.as_os_str().is_empty()) {
        return None;
    }
    let home = home?;
    let has_startup_file = [".zshenv", ".zprofile", ".zshrc", ".zlogin"]
        .iter()
        .any(|name| std::fs::symlink_metadata(home.join(name)).is_ok());
    if has_startup_file {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("commandui-zdotdir-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir(&dir).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok()?;
    }
    let dir_text = dir.to_string_lossy().replace('\'', "'\\''");
    let script = format!("unset ZDOTDIR\nskip_global_compinit=1\ncommand rm -rf -- '{dir_text}'\n");
    std::fs::write(dir.join(".zshenv"), script).ok()?;
    Some(dir)
}

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkerKind {
    /// A prompt was drawn: the shell is idle. Carries the exit code of the
    /// command that ended, when the shell knows it.
    Prompt,
    /// cmd only: the exit code of the command that just ran, written before
    /// its prompt.
    Exit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Marker {
    pub kind: MarkerKind,
    pub nonce: String,
    pub exit: Option<i32>,
    /// Whether the shell reports its clear-line chord as bound.
    pub chord: Option<bool>,
    /// Decoded; empty for an exit marker.
    pub cwd: String,
}

/// Undo `encode_cwd`: every `%HH` becomes that byte; the result is read as
/// UTF-8 (lossily). A `%` not followed by two hex digits is kept as written.
pub(crate) fn decode_cwd(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What the shell hooks do to a cwd: `%`, `;`, ESC, BEL, CR and LF are written
/// as `%25`, `%3B`, `%1B`, `%07`, `%0D`, `%0A`.
#[cfg(test)]
pub(crate) fn encode_cwd(cwd: &str) -> String {
    let mut out = String::with_capacity(cwd.len());
    for c in cwd.chars() {
        match c {
            '%' => out.push_str("%25"),
            ';' => out.push_str("%3B"),
            '\x1b' => out.push_str("%1B"),
            '\x07' => out.push_str("%07"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            c => out.push(c),
        }
    }
    out
}

/// Parse the part of a marker after `ESC ] 7733 ;`. Anything malformed is not
/// a marker: a bad exit code, an unknown kind, a missing field.
pub(crate) fn parse_marker(body: &str) -> Option<Marker> {
    let mut fields = body.splitn(5, ';');
    let kind = match fields.next()? {
        "P" => MarkerKind::Prompt,
        "X" => MarkerKind::Exit,
        _ => return None,
    };
    let nonce = fields.next()?;
    if nonce.is_empty() {
        return None;
    }
    let exit = match fields.next()? {
        "" => None,
        code => Some(code.parse::<i32>().ok()?),
    };
    let chord = match fields.next()? {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    };
    let cwd = decode_cwd(fields.next()?);
    Some(Marker { kind, nonce: nonce.to_string(), exit, chord, cwd })
}

/// A marker as the hooks write it, for tests.
#[cfg(test)]
pub(crate) fn marker_osc(kind: char, nonce: &str, exit: Option<i32>, chord: Option<bool>, cwd: &str) -> String {
    let exit = exit.map(|c| c.to_string()).unwrap_or_default();
    let chord = match chord {
        Some(true) => "1",
        Some(false) => "0",
        None => "",
    };
    format!("{OSC_MARKER_OPEN}{kind};{nonce};{exit};{chord};{}\x07", encode_cwd(cwd))
}

/// What the reader hands on, in stream order.
#[derive(Debug, PartialEq)]
pub(crate) enum ReaderEvent {
    Text(String),
    Marker(Marker),
    /// The stream has been quiet for `HELD_BREAK_FLUSH`: whatever a consumer
    /// holds back waiting for the rest of a split sequence can be released.
    /// Sent once per quiet period.
    Idle,
}

/// The longest OSC the scanner waits for the end of. A longer one is not a
/// marker and is released as text.
const MAX_OSC_LEN: usize = 8192;

/// Takes marker sequences out of the text stream. Everything else, including
/// every other escape sequence, passes through in order. An OSC cut in half by
/// a read is held for the next one (bounded by `MAX_OSC_LEN`).
///
/// Window-title OSCs (`0`, `1`, `2`) are dropped as well: cmd retitles its
/// console with the command line it is running, which would show the plumbing.
#[derive(Default)]
pub(crate) struct MarkerScanner {
    carry: String,
}

/// Where an OSC that starts at `rest[0]` (`ESC ]`) ends: `(end of body, end of
/// sequence)`, or None when it is not complete yet. A line break or an ESC that
/// is not the start of ST also ends one, as in a terminal; neither is part of
/// the sequence.
fn osc_end(rest: &str) -> Option<(usize, usize)> {
    let b = rest.as_bytes();
    let mut j = 2;
    while j < b.len() {
        match b[j] {
            0x07 => return Some((j, j + 1)),
            0x1b if b.get(j + 1) == Some(&b'\\') => return Some((j, j + 2)),
            0x1b if j + 1 >= b.len() => return None,
            // Any other ESC starts a new sequence and aborts this one, as in a
            // terminal: a half-received OSC cannot swallow the marker after it.
            0x1b | b'\r' | b'\n' => return Some((j, j)),
            _ => j += 1,
        }
    }
    None
}

impl MarkerScanner {
    pub(crate) fn push(&mut self, text: &str) -> Vec<ReaderEvent> {
        let mut input = std::mem::take(&mut self.carry);
        input.push_str(text);
        let mut events = Vec::new();
        let mut plain = String::new();
        let mut i = 0;
        while i < input.len() {
            let Some(rel) = input[i..].find('\x1b') else {
                plain.push_str(&input[i..]);
                break;
            };
            plain.push_str(&input[i..i + rel]);
            i += rel;
            let rest = &input[i..];
            if rest.len() == 1 {
                // A lone ESC at the end of the read: the next one says what it is.
                self.carry = rest.to_string();
                break;
            }
            if !rest.starts_with("\x1b]") {
                plain.push('\x1b');
                i += 1;
                continue;
            }
            let Some((body_end, total)) = osc_end(rest) else {
                if rest.len() > MAX_OSC_LEN {
                    plain.push('\x1b');
                    i += 1;
                    continue;
                }
                self.carry = rest.to_string();
                break;
            };
            let body = &rest[2..body_end];
            if let Some(marker_body) = body.strip_prefix("7733;") {
                if !plain.is_empty() {
                    events.push(ReaderEvent::Text(std::mem::take(&mut plain)));
                }
                // A malformed marker is dropped, never shown.
                if let Some(marker) = parse_marker(marker_body) {
                    events.push(ReaderEvent::Marker(marker));
                }
            } else if body.starts_with("0;") || body.starts_with("1;") || body.starts_with("2;") {
                // A window title: not output.
            } else {
                plain.push_str(&rest[..total]);
            }
            i += total;
        }
        if !plain.is_empty() {
            events.push(ReaderEvent::Text(plain));
        }
        events
    }

    /// Whatever is still held when the stream ends. A half-received marker or
    /// title is dropped; anything else is text.
    pub(crate) fn finish(&mut self) -> String {
        let carry = std::mem::take(&mut self.carry);
        if carry.starts_with("\x1b]") {
            let body = &carry[2..];
            let ours = OSC_MARKER_OPEN[2..].starts_with(body) || body.starts_with("7733;");
            let title = body.starts_with("0;") || body.starts_with("1;") || body.starts_with("2;");
            if ours || title {
                return String::new();
            }
        }
        carry
    }
}

// ---------------------------------------------------------------------------
// Shell hooks
// ---------------------------------------------------------------------------

/// Name of the cmd variable holding an ESC character.
const CMD_ESC_VAR: &str = "__cue";
/// Name of the cmd variable that holds the exit-code tail.
const CMD_TAIL_VAR: &str = "__cui";
/// Name of the cmd variable holding `(call )`, which sets ERRORLEVEL to 0.
/// cmd's built-ins (echo, cd, set) never reset ERRORLEVEL, and the tail's own
/// `set /p` leaves it at 1, so without it an approved `echo` would report
/// whatever the command before it left behind.
const CMD_RESET_VAR: &str = "__cuz";

/// What the console echoes in front of an executed cmd command: the
/// ERRORLEVEL reset. It is dropped from what is shown.
pub(crate) const CMD_RESET_ECHO: &str = "%__cuz% & ";
/// What it echoes after an executed cmd command: the exit-code tail.
pub(crate) const CMD_TAIL_ECHO: &str = " & %__cui%";
/// The tail alone, as a line of its own (the probe after a command that could
/// not carry it).
pub(crate) const CMD_PROBE_ECHO: &str = "%__cui%";

const CMD_PLUMBING: [&str; 3] = [CMD_RESET_ECHO, CMD_TAIL_ECHO, CMD_PROBE_ECHO];

/// Take the cmd plumbing the console echoes out of display text. `hold` keeps
/// the tail of the text that could still turn into plumbing once more arrives
/// (the console echoes a line in pieces: `%__` then `cuz% & dir`). With
/// `flush` nothing is held. Only used for cmd.
#[cfg(test)]
pub(crate) fn strip_cmd_plumbing(hold: &mut String, text: &str, flush: bool) -> String {
    strip_cmd_echo(&mut Vec::new(), hold, text, flush)
}

/// The lines the runtime types into cmd to set it up, as the console echoes
/// them (without the line ending).
pub(crate) fn cmd_bootstrap_echo(bootstrap: &str) -> Vec<String> {
    bootstrap.split(ENTER).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// `strip_cmd_plumbing`, and also the echo of each of `boot_lines` (the
/// bootstrap the runtime typed), each taken out once. The console can paint
/// that echo after the session is ready, in pieces.
pub(crate) fn strip_cmd_echo(boot_lines: &mut Vec<String>, hold: &mut String, text: &str, flush: bool) -> String {
    let mut s = std::mem::take(hold);
    s.push_str(text);
    boot_lines.retain(|line| {
        if let Some(at) = s.find(line.as_str()) {
            s.replace_range(at..at + line.len(), "");
            false
        } else {
            true
        }
    });
    for plumbing in CMD_PLUMBING {
        if s.contains(plumbing) {
            s = s.replace(plumbing, "");
        }
    }
    if !flush {
        // The longest suffix that is a proper prefix of some string to remove.
        let mut keep = 0;
        for plumbing in CMD_PLUMBING.iter().copied().chain(boot_lines.iter().map(String::as_str)) {
            for len in (1..plumbing.len()).rev() {
                if len > keep && len <= s.len() && s.is_char_boundary(s.len() - len) && plumbing.is_char_boundary(len) && s.ends_with(&plumbing[..len]) {
                    keep = len;
                    break;
                }
            }
        }
        if keep > 0 {
            *hold = s.split_off(s.len() - keep);
        }
    }
    s
}

fn bootstrap_powershell(nonce: &str) -> String {
    // The prompt function writes the marker with [Console]::Write, as one
    // write per prompt (a string returned from `prompt` would be measured by
    // PSReadLine as part of the prompt and drawn again on a full redraw).
    //
    // The cwd is written as UTF-8 bytes, every byte outside printable ASCII
    // (and `%` and `;`) as `%HH`: [Console]::Write encodes with the console's
    // output code page, which turns a non-ASCII character into `?`.
    //
    // The clear chord (Ctrl+]) is bound once per editing mode (PSReadLine's
    // default there is GotoBrace or CharacterSearch, which swallows the next
    // key), only when the chord is unbound or still has its default, and read
    // back; the marker says whether it took. It is not re-bound at every
    // prompt, so a binding the user makes later is left alone.
    let script = r#"$global:__cui_n = '@NONCE@'; $global:__cui_mode = $null; $global:__cui_chord = '0'; function global:prompt { $__cui_ok = $?; $__cui_code = $global:LASTEXITCODE; if ($__cui_ok) { $__cui_code = 0 } elseif (-not ($__cui_code -is [int]) -or $__cui_code -eq 0) { $__cui_code = 1 }; try { $__cui_m = [string](Get-PSReadLineOption).EditMode; if ($__cui_m -ne $global:__cui_mode) { $global:__cui_mode = $__cui_m; $__cui_h = @(Get-PSReadLineKeyHandler -Bound | Where-Object { $_.Key -eq 'Ctrl+]' } | ForEach-Object { [string]$_.Function }); if (@($__cui_h | Where-Object { $_ -notin 'GotoBrace','CharacterSearch','RevertLine' }).Count -eq 0) { if ($__cui_m -eq 'Vi') { Set-PSReadLineKeyHandler -ViMode Insert -Chord 'Ctrl+]' -Function RevertLine; Set-PSReadLineKeyHandler -ViMode Command -Chord 'Ctrl+]' -ScriptBlock { [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine(); [Microsoft.PowerShell.PSConsoleReadLine]::ViInsertMode() } } else { Set-PSReadLineKeyHandler -Chord 'Ctrl+]' -Function RevertLine }; $global:__cui_chord = if (@(Get-PSReadLineKeyHandler -Bound | Where-Object { $_.Key -eq 'Ctrl+]' -and [string]$_.Function -eq 'RevertLine' }).Count -gt 0) { '1' } else { '0' } } else { $global:__cui_chord = '0' } } } catch { $global:__cui_chord = '0' }; $__cui_cwd = -join ([System.Text.Encoding]::UTF8.GetBytes((Get-Location).Path) | ForEach-Object { if ($_ -lt 32 -or $_ -gt 126 -or $_ -eq 37 -or $_ -eq 59) { '%{0:X2}' -f $_ } else { [string][char]$_ } }); [Console]::Write([string][char]27 + ']7733;P;' + $global:__cui_n + ';' + $__cui_code + ';' + $global:__cui_chord + ';' + $__cui_cwd + [string][char]7); '> ' }"#;
    format!("{}{ENTER}", script.replace("@NONCE@", nonce))
}

fn bootstrap_bash(nonce: &str) -> String {
    // `bind` is only used when Ctrl+] is unbound or still readline's default
    // (character-search), in all three keymaps, and the result is read back.
    // The marker carries the answer; a runtime that is told 0 clears a line
    // with Ctrl+E Ctrl+U instead.
    let script = r#"__cui_nonce='@NONCE@'; __cui_chord=0; __cui_prompt() { local ec=$? cwd=$PWD; cwd=${cwd//\%/%25}; cwd=${cwd//;/%3B}; cwd=${cwd//$'\e'/%1B}; cwd=${cwd//$'\a'/%07}; cwd=${cwd//$'\n'/%0A}; cwd=${cwd//$'\r'/%0D}; printf '\033]7733;P;%s;%s;%s;%s\007' "$__cui_nonce" "$ec" "$__cui_chord" "$cwd"; }; __cui_free() { local l; l=$( { bind -m "$1" -p; bind -m "$1" -s; bind -m "$1" -X; } 2>/dev/null | grep -F '"\C-]":' ); [ -z "$l" ] || [ "$l" = '"\C-]": character-search' ] || [ "$l" = '"\C-]": self-insert' ]; }; if __cui_free emacs && __cui_free vi-insert && __cui_free vi-command; then bind -m emacs '"\C-]": kill-whole-line' 2>/dev/null; bind -m vi-insert '"\C-]": kill-whole-line' 2>/dev/null; bind -m vi-command '"\C-]": "A\C-u"' 2>/dev/null; if bind -m emacs -p 2>/dev/null | grep -qF '"\C-]": kill-whole-line' && bind -m vi-insert -p 2>/dev/null | grep -qF '"\C-]": kill-whole-line' && bind -m vi-command -s 2>/dev/null | grep -qF '"\C-]": "A\C-u"'; then __cui_chord=1; fi; fi; PROMPT_COMMAND=__cui_prompt"#;
    format!("{}{ENTER}", powershell_script(nonce, script))
}

fn powershell_script(nonce: &str, script: &str) -> String {
    script.replace("@NONCE@", nonce)
}

/// Arguments that start a shell already set up, so the runtime types nothing
/// and the console echoes nothing. PowerShell takes its bootstrap as
/// `-EncodedCommand` (with `-NoExit` it stays interactive, after the profile
/// has run): typed in, PSReadLine echoed the whole script, coloured token by
/// token, before the first prompt, and no string match could take it out
/// again. The other shells keep the typed bootstrap; an empty list means
/// "type `bootstrap_prompt`".
pub fn launch_args(shell: &str, nonce: &str) -> Vec<String> {
    match shell_family(shell) {
        ShellFamily::PowerShell => {
            let script = bootstrap_powershell(nonce);
            let script = script.strip_suffix(ENTER).unwrap_or(&script);
            vec![
                "-NoLogo".to_string(),
                "-NoExit".to_string(),
                "-EncodedCommand".to_string(),
                encode_powershell_command(script),
            ]
        }
        _ => Vec::new(),
    }
}

/// PowerShell's `-EncodedCommand` form: base64 of the UTF-16LE script. No
/// quoting rules apply to it, so the script reaches PowerShell byte for byte.
pub(crate) fn encode_powershell_command(script: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The folder a new session starts in when the caller names none: the
/// process's folder, unless that is the Windows folder or one under it. A
/// packaged (MSIX) app started from Start or the taskbar runs in
/// `C:\Windows\System32`; a terminal opening there is never what the user
/// wants, so the session starts in their home folder instead. Also the
/// fallback when the process folder cannot be read.
pub fn default_session_cwd(
    process_dir: Option<std::path::PathBuf>,
    home: Option<std::path::PathBuf>,
    windows_dir: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    let under_windows = |dir: &std::path::Path| {
        windows_dir.as_deref().is_some_and(|win| {
            let (d, w) = (dir.to_string_lossy().to_lowercase(), win.to_string_lossy().to_lowercase());
            let w = w.trim_end_matches(['\\', '/']);
            !w.is_empty() && (d == w || d.starts_with(&format!("{w}\\")) || d.starts_with(&format!("{w}/")))
        })
    };
    match process_dir {
        Some(dir) if !under_windows(&dir) => Some(dir),
        other => home.or(other),
    }
}

/// `default_session_cwd` for this process: its folder, the user's home
/// folder (`USERPROFILE`, else `HOME`) and the Windows folder (`SystemRoot`).
pub fn resolve_default_session_cwd() -> Option<std::path::PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    default_session_cwd(
        std::env::current_dir().ok(),
        var("USERPROFILE").or_else(|| var("HOME")),
        var("SystemRoot").or_else(|| var("windir")),
    )
}

fn bootstrap_zsh(nonce: &str) -> String {
    let script = r#"precmd() { local __cui_ec=$? __cui_cwd=${PWD}; __cui_cwd=${__cui_cwd//\%/%25}; __cui_cwd=${__cui_cwd//;/%3B}; __cui_cwd=${__cui_cwd//$'\e'/%1B}; __cui_cwd=${__cui_cwd//$'\a'/%07}; __cui_cwd=${__cui_cwd//$'\n'/%0A}; __cui_cwd=${__cui_cwd//$'\r'/%0D}; print -rn -- $'\e]7733;P;@NONCE@;'"${__cui_ec}"';;'"${__cui_cwd}"$'\a' }"#;
    format!("{}{ENTER}", script.replace("@NONCE@", nonce))
}

fn bootstrap_cmd(nonce: &str) -> String {
    // 1. An ESC character in %__cue% (the classic `prompt $E` capture).
    // 2. `(call )`, which sets ERRORLEVEL to 0.
    // 3. The tail that reports the exit code of the command before it: `call`
    //    expands %ERRORLEVEL% a second time, after the command has run, and
    //    `set /p` with no input writes the OSC without a line break.
    // 4. cmd's own prompt, which writes the prompt marker before every prompt,
    //    typed or approved. PROMPT cannot expand ERRORLEVEL, which is why 3
    //    exists. Last, so the first prompt that carries a marker is the one
    //    after the whole bootstrap.
    let esc = CMD_ESC_VAR;
    format!(
        "for /F \"tokens=1,2 delims=#\" %a in ('\"prompt #$H#$E# & echo on & for %b in (1) do rem\"') do @set \"{esc}=%b\"{ENTER}\
         @set \"{CMD_RESET_VAR}=(call )\"{ENTER}\
         @set \"{CMD_TAIL_VAR}=call <nul set /p=%{esc}%]7733;X;{nonce};%^ERRORLEVEL%;;%{esc}%\\\"{ENTER}\
         @prompt $e]7733;P;{nonce};;;$P$e\\$P$G{ENTER}"
    )
}

/// The prompt hook for a shell: typed into the shell once it has started.
pub fn bootstrap_prompt(shell: &str, nonce: &str) -> Option<String> {
    match shell_family(shell) {
        ShellFamily::PowerShell => Some(bootstrap_powershell(nonce)),
        ShellFamily::Cmd => Some(bootstrap_cmd(nonce)),
        ShellFamily::Bash => Some(bootstrap_bash(nonce)),
        ShellFamily::Zsh => Some(bootstrap_zsh(nonce)),
        ShellFamily::Unsupported => None,
    }
}

/// How an approved cmd line ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmdTail {
    /// ` & %__cui%` on the same line reports the exit code before the prompt.
    Chained,
    /// The command could swallow or break the tail (a trailing `rem`, `::`,
    /// `^`, an open quote or paren, an if/for, a trailing operator): the line
    /// is the command alone. Its prompt comes without an exit code, and the
    /// runtime then sends the tail as a line of its own once the shell is idle.
    None,
}

/// Bytes written for one executed command, and how it ends. cmd chains an
/// exit-code tail after the command on the same line because its prompt cannot
/// expand ERRORLEVEL. The command text is the command, not a format string.
pub(crate) fn command_line_for_shell(shell: &str, command: &str, chord: bool) -> (String, CmdTail) {
    let clear = clear_input_line(shell, chord);
    if shell_family(shell) == ShellFamily::Cmd {
        if cmd_can_chain(command) {
            (format!("{clear}%{CMD_RESET_VAR}% & {command} & %{CMD_TAIL_VAR}%{ENTER}"), CmdTail::Chained)
        } else {
            (format!("{clear}%{CMD_RESET_VAR}% & {command}{ENTER}"), CmdTail::None)
        }
    } else {
        (format!("{clear}{command}{ENTER}"), CmdTail::Chained)
    }
}

/// The follow-up line for a cmd command that could not carry the tail: the
/// tail on its own, written once the shell is idle at a prompt (ERRORLEVEL is
/// still what the command left).
pub(crate) fn cmd_probe_line(chord: bool) -> String {
    format!("{}%{CMD_TAIL_VAR}%{ENTER}", clear_input_line("cmd.exe", chord))
}

/// Can ` & tail` be appended to this cmd line and still run once, always,
/// after the command? Not when the command could swallow or break it.
fn cmd_can_chain(command: &str) -> bool {
    if command.matches('"').count() % 2 != 0 {
        return false;
    }
    let trimmed = command.trim_end();
    // A trailing operator or redirection: `echo a &` works alone, but
    // `echo a & & tail` does not parse, and `echo a >` has no target.
    if trimmed.ends_with(['^', '&', '|', '<', '>']) {
        return false;
    }
    let mut depth: i32 = 0;
    let mut quoted = false;
    for c in command.chars() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    if depth != 0 {
        return false;
    }
    !cmd_segment_swallows_tail(command)
}

/// Is `if`, `for`, `rem` or `::` the first word of a command segment (the start
/// of the line, or after an unquoted `&`, `|` or `(`)? An `if` or `for` takes
/// the rest of the line into its body and a `rem` or `::` comments it out, tail
/// included. The same word elsewhere (`echo for you`, a quoted `"fix for bug"`,
/// `findstr if x`) is an argument and takes nothing.
fn cmd_segment_swallows_tail(command: &str) -> bool {
    let mut quoted = false;
    let mut at_start = true;
    let mut word = String::new();
    let mut chars = command.chars().peekable();
    // Finish a word that began a segment.
    let swallows = |w: &str| matches!(w.to_ascii_lowercase().as_str(), "if" | "for" | "rem");
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => {
                quoted = true;
                at_start = false;
            }
            '&' | '|' | '(' => {
                if at_start && swallows(&word) {
                    return true;
                }
                word.clear();
                at_start = true;
            }
            ' ' | '\t' if at_start && word.is_empty() => {}
            '@' if at_start && word.is_empty() => {}
            ':' if at_start && word.is_empty() && chars.peek() == Some(&':') => return true,
            c if at_start && (c.is_ascii_alphanumeric()) => word.push(c),
            _ => {
                if at_start && swallows(&word) {
                    return true;
                }
                word.clear();
                at_start = false;
            }
        }
    }
    at_start && swallows(&word)
}

/// The key chord that clears the input line whatever the editor is doing. The
/// shell binds it at bootstrap, only when it is unbound or still the default,
/// in every editing mode, to "throw away the line and be ready to insert":
/// PowerShell binds it to RevertLine (once per editing mode; vi command mode
/// gets a handler that also returns to insert), bash binds it with `bind` in
/// the emacs, vi-insert and vi-command keymaps. A vi command mode has no key
/// that clears a line, so no fixed run of ordinary keys could work there. The
/// marker tells the runtime whether the binding took (see `Marker::chord`).
pub(crate) const CLEAR_CHORD: &str = "\x1d";

/// Bytes that discard whatever the user already typed at the prompt, written
/// in the same write as an approved command so it cannot be appended to a
/// half-typed line (`rm -rf ` + approved `ls` must not run `rm -rf ls`).
///
/// bash and PowerShell: the bound chord above when the shell says it took
/// (PowerShell then also gets Ctrl+End, Ctrl+Home for a console without
/// PSReadLine); otherwise PowerShell gets only the Ctrl+End, Ctrl+Home pair
/// and bash falls back to Ctrl+E, Ctrl+U (which a vi command mode cannot use).
/// zsh (zle): Ctrl+E (end of line) then Ctrl+U (kill to start).
///
/// cmd: Ctrl+End then Ctrl+Home, as the VT input sequences `CSI 1;5 F` and
/// `CSI 1;5 H`; its console line editor deletes to the end / start of the
/// line, so the whole line goes whatever the cursor position. Escape is NOT
/// usable: a ConPTY reads ESC followed by more bytes in the same write as
/// Alt+<key>, so the first letter of the command was swallowed (`xyz` + ESC +
/// `echo ok` ran `xyzecho ok`). The CSI forms are unambiguous in a single write.
pub(crate) fn clear_input_line(shell: &str, chord: bool) -> String {
    match shell_family(shell) {
        ShellFamily::Bash if chord => CLEAR_CHORD.to_string(),
        ShellFamily::Bash => "\x05\x15".to_string(),
        ShellFamily::Zsh => "\x05\x15".to_string(),
        ShellFamily::PowerShell if chord => format!("{CLEAR_CHORD}\x1b[1;5F\x1b[1;5H"),
        ShellFamily::PowerShell => "\x1b[1;5F\x1b[1;5H".to_string(),
        ShellFamily::Cmd => "\x1b[1;5F\x1b[1;5H".to_string(),
        ShellFamily::Unsupported => String::new(),
    }
}

/// What is written to make a session report a prompt again: the line is
/// cleared first (a half-typed line must not be submitted by the Enter), then
/// Enter draws a fresh prompt.
pub(crate) fn resync_input(shell: &str, chord: bool) -> String {
    format!("{}{ENTER}", clear_input_line(shell, chord))
}

/// Number of lines a write submits: each CR, LF, or CR LF counts once, outside
/// a bracketed paste (`ESC [ 200 ~ ... ESC [ 201 ~`), where a shell inserts
/// line breaks instead of running them. A paste whose end has not arrived
/// counts nothing after its start.
pub(crate) fn submitted_lines(data: &str) -> u32 {
    const START: &str = "\x1b[200~";
    const END: &str = "\x1b[201~";
    let mut outside = String::new();
    let mut rest = data;
    loop {
        match rest.find(START) {
            None => {
                outside.push_str(rest);
                break;
            }
            Some(start) => {
                outside.push_str(&rest[..start]);
                match rest[start + START.len()..].find(END) {
                    Some(end) => rest = &rest[start + START.len() + end + END.len()..],
                    None => break,
                }
            }
        }
    }
    let mut count = 0;
    let mut prev_cr = false;
    for c in outside.chars() {
        match c {
            '\r' => count += 1,
            '\n' if !prev_cr => count += 1,
            _ => {}
        }
        prev_cr = c == '\r';
    }
    count
}

// ---------------------------------------------------------------------------
// Display rewrite (Windows)
// ---------------------------------------------------------------------------

/// A Windows ConPTY repaints by position: instead of `\r\n` it moves to the
/// start of the next row with a cursor-position escape (`CSI row;col H`), so a
/// finished line (the output of a command) can arrive with no line break at
/// all. A consumer that shows output as lines (the console transcript) needs
/// the break, so this turns such a cursor-position escape into a line break
/// when the row it leaves has text on it and the cursor moves to a different
/// row, and drops it otherwise. A cursor-position escape that stays on the row
/// the cursor is already on (an in-line redraw) passes through, as does every
/// other escape. The row is followed from the escapes seen and from line feeds
/// (a ConPTY soft wrap is not a new row: see "Soft wraps" below; an unknown row
/// counts as different).
/// While the alternate screen is active (`CSI ? 1049/1047/47 h`, alone or
/// combined with other modes) nothing is rewritten: vim, less and htop need
/// their cursor positioning. A prompt marker ends it (`end_alt_screen`): a
/// program that was killed never sent its `?1049l`.
/// An escape sequence cut in half by a read is held for the next one.
///
/// **This is display only.** Prompt markers are OSC sequences taken out of the
/// stream before the text gets here, so whether a row was wrapped, split or
/// repainted has no bearing on whether a command finishes. A wrong guess here
/// shows a long output line as two, or two as one.
///
/// Soft wraps. When a row is filled to the last column of a console that is
/// already at the bottom of its screen, ConPTY does not leave the wrap to the
/// terminal: it writes `CR LF` (to scroll), moves back to the last column of
/// the row that was wrapped (`CSI row;cols H`) and repaints the last character
/// before continuing. That is one long line, not two. A line break is
/// therefore not a line break when the row it ends was filled up to column N
/// and the next cursor-position escape (after only colour or cursor-visibility
/// escapes) lands on column N: the break and the escape are dropped, and the
/// repainted character, which is the character already shown, is dropped
/// too. The column is followed in display cells from the text written (a wide
/// character takes two), and the console width is seeded from the pty size and
/// follows resizes, so a line of several rows is recognised too. A `CR` or `LF`
/// on a row this wide is held until the next read shows which it is
/// (`flush_held` releases it when the reader has been idle for
/// `HELD_BREAK_FLUSH`, and a prompt marker releases it at once).
#[derive(Default)]
pub(crate) struct RowNormalizer {
    carry: String,
    row_has_text: bool,
    alt_screen: bool,
    row: Option<u32>,
    /// Cursor column, 0-based: how many cells are to the left of it.
    col: usize,
    /// The column a carriage return left, for the line feed that follows it.
    col_before_cr: usize,
    last_char: Option<char>,
    /// After a soft wrap: the character ConPTY repaints, to be dropped once.
    drop_dup: Option<char>,
    /// `carry` starts with a CR/LF that waits for the next read.
    holding: bool,
    /// The console width in cells. Text written past it continues on the next
    /// row, as the terminal does.
    width: Option<usize>,
}

/// Rows narrower than this are never treated as possibly soft-wrapped.
const MIN_WRAP_COL: usize = 8;

enum Wrap {
    No,
    More,
    Yes { pass: (usize, usize), end: usize, row: Option<u32>, col: usize },
}

/// Is the line break at `i` (`CR LF` or `LF`) a soft wrap of a row filled to
/// column `cand`? See `RowNormalizer`. With no known width, a column that
/// divides `cand` counts too (a logical line of several rows).
fn wrap_lookahead(input: &str, i: usize, cand: usize, width_known: bool) -> Wrap {
    let b = input.as_bytes();
    let mut j = i;
    if b.get(j) == Some(&b'\r') {
        j += 1;
    }
    match b.get(j) {
        None => return Wrap::More,
        Some(b'\n') => j += 1,
        Some(_) => return Wrap::No,
    }
    let pass_start = j;
    loop {
        match b.get(j) {
            None => return Wrap::More,
            Some(0x1b) => {
                match b.get(j + 1) {
                    None => return Wrap::More,
                    Some(b'[') => {}
                    Some(_) => return Wrap::No,
                }
                let mut k = j + 2;
                while k < b.len() && (0x20..=0x3f).contains(&b[k]) {
                    k += 1;
                }
                let Some(&fin) = b.get(k) else { return Wrap::More };
                let params = &input[j + 2..k];
                match fin {
                    b'm' => j = k + 1,
                    b'h' | b'l' if params == "?25" => j = k + 1,
                    b'H' | b'f' => {
                        let mut it = params.split(';');
                        let row = it.next().and_then(|r| if r.is_empty() { Some(1) } else { r.parse::<u32>().ok() });
                        let col = it.next().and_then(|c| if c.is_empty() { Some(1) } else { c.parse::<usize>().ok() });
                        return match col {
                            Some(c) if c > 1 && (c == cand || (!width_known && cand >= c && cand % c == 0)) => {
                                Wrap::Yes { pass: (pass_start, j), end: k + 1, row, col: c }
                            }
                            _ => Wrap::No,
                        };
                    }
                    _ => return Wrap::No,
                }
            }
            Some(_) => return Wrap::No,
        }
    }
}

/// The 0-based column a CSI sequence leaves the cursor in, when it says.
fn csi_column(seq: &str, col: usize) -> usize {
    let Some(fin) = seq.bytes().last() else { return col };
    let params = &seq[2..seq.len() - 1];
    let n = |p: &str| p.split(';').next().and_then(|v| if v.is_empty() { Some(1) } else { v.parse::<usize>().ok() });
    match fin {
        b'H' | b'f' => params
            .split(';')
            .nth(1)
            .and_then(|c| if c.is_empty() { Some(1) } else { c.parse::<usize>().ok() })
            .map(|c| c.max(1) - 1)
            .unwrap_or(0),
        b'G' => n(params).map(|c| c.max(1) - 1).unwrap_or(col),
        b'C' => col + n(params).unwrap_or(1),
        b'D' => col.saturating_sub(n(params).unwrap_or(1)),
        b'E' | b'F' => 0,
        _ => col,
    }
}

const MAX_ESCAPE_CARRY: usize = 256;

/// How many cells a character takes: 0 for combining marks and joiners, 2 for
/// the East Asian Wide and Fullwidth blocks and emoji, else 1. A small table,
/// enough for column tracking; a wrong guess here is cosmetic.
fn char_cells(c: char) -> usize {
    let u = c as u32;
    if u < 0x300 {
        return 1;
    }
    if matches!(
        u,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x200B..=0x200F | 0x2060..=0x2064
            | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0xE0100..=0xE01EF
    ) {
        return 0;
    }
    if matches!(
        u,
        0x1100..=0x115F | 0x231A..=0x231B | 0x2329..=0x232A | 0x23E9..=0x23EC | 0x25FD..=0x25FE
            | 0x2614..=0x2615 | 0x2648..=0x2653 | 0x267F | 0x2693 | 0x26A1 | 0x26AA..=0x26AB
            | 0x26BD..=0x26BE | 0x26C4..=0x26C5 | 0x26CE | 0x26D4 | 0x26EA | 0x26F2..=0x26F3
            | 0x26F5 | 0x26FA | 0x26FD | 0x2705 | 0x270A..=0x270B | 0x2728 | 0x274C | 0x274E
            | 0x2753..=0x2755 | 0x2757 | 0x2795..=0x2797 | 0x27B0 | 0x27BF | 0x2B1B..=0x2B1C
            | 0x2B50 | 0x2B55 | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xA960..=0xA97F | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF | 0xFE10..=0xFE19 | 0xFE30..=0xFE6F | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6 | 0x1F300..=0x1F64F | 0x1F680..=0x1F6FF | 0x1F900..=0x1F9FF
            | 0x1FA70..=0x1FAFF | 0x20000..=0x3FFFD
    ) {
        return 2;
    }
    1
}

/// `Some(true)` / `Some(false)` when `seq` is a private-mode set / reset that
/// includes the alternate screen (1049, 1047 or 47), alone or combined.
fn alt_screen_switch(seq: &str) -> Option<bool> {
    let body = seq.strip_prefix("\x1b[?")?;
    let (params, on) = if let Some(p) = body.strip_suffix('h') {
        (p, true)
    } else {
        (body.strip_suffix('l')?, false)
    };
    params
        .split(';')
        .any(|p| matches!(p, "1049" | "1047" | "47"))
        .then_some(on)
}

impl RowNormalizer {
    /// A normalizer for a console `cols` wide.
    pub(crate) fn with_width(cols: u16) -> Self {
        let mut rows = Self::default();
        rows.set_width(cols);
        rows
    }

    /// The console was resized (or is being seeded).
    pub(crate) fn set_width(&mut self, cols: u16) {
        self.width = (cols as usize >= MIN_WRAP_COL).then_some(cols as usize);
    }

    pub(crate) fn push(&mut self, text: &str) -> String {
        self.push_inner(text, false)
    }

    /// Release a CR/LF that was held for the next read, now that no read came.
    /// Returns "" when nothing is held.
    pub(crate) fn flush_held(&mut self) -> String {
        if self.holding {
            self.push_inner("", true)
        } else {
            String::new()
        }
    }

    /// A prompt was drawn, so no full-screen program is running: leave the
    /// alternate screen, and start the prompt on a row of its own if the
    /// screen's text is still on this one.
    pub(crate) fn end_alt_screen(&mut self) -> String {
        let was = self.alt_screen;
        self.alt_screen = false;
        if was && self.row_has_text {
            self.row_has_text = false;
            self.col = 0;
            return "\n".to_string();
        }
        String::new()
    }

    fn push_inner(&mut self, text: &str, force: bool) -> String {
        let mut input = std::mem::take(&mut self.carry);
        self.holding = false;
        input.push_str(text);
        let bytes = input.as_bytes();
        let mut out = String::with_capacity(input.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != 0x1b {
                let rest = &input[i..];
                let ch = rest.chars().next().unwrap();
                if (ch == '\r' || ch == '\n') && !self.alt_screen {
                    let cand = if ch == '\n' && self.col == 0 { self.col_before_cr } else { self.col };
                    if cand >= MIN_WRAP_COL {
                        match wrap_lookahead(&input, i, cand, self.width.is_some()) {
                            Wrap::Yes { pass, end, row, col } => {
                                out.push_str(&input[pass.0..pass.1]);
                                i = end;
                                self.row = row;
                                self.row_has_text = true;
                                self.col = col - 1;
                                self.width = Some(col);
                                self.drop_dup = self.last_char;
                                continue;
                            }
                            Wrap::More if !force => {
                                self.carry = input[i..].to_string();
                                self.holding = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                if ch == '\n' {
                    self.row_has_text = false;
                    self.row = self.row.map(|r| r + 1);
                    self.col = 0;
                    self.drop_dup = None;
                } else if ch == '\r' {
                    if self.col > 0 {
                        self.col_before_cr = self.col;
                    }
                    self.col = 0;
                    self.drop_dup = None;
                } else if ch == '\u{8}' {
                    self.col = self.col.saturating_sub(1);
                } else if ch == '\t' {
                    self.col = (self.col / 8 + 1) * 8;
                } else if !ch.is_control() {
                    let cells = char_cells(ch);
                    if self.drop_dup.take() == Some(ch) {
                        self.col += cells;
                        i += ch.len_utf8();
                        continue;
                    }
                    if self.width.is_some_and(|w| self.col >= w) {
                        self.col = 0;
                    }
                    self.col += cells;
                    self.col_before_cr = 0;
                    self.last_char = Some(ch);
                }
                if !ch.is_whitespace() && !ch.is_control() {
                    self.row_has_text = true;
                }
                out.push(ch);
                i += ch.len_utf8();
                continue;
            }
            // An escape sequence starts here; find where it ends. Every end
            // offset is just after an ASCII byte (or at one), so slicing the
            // str there is always on a character boundary.
            let end = match bytes.get(i + 1) {
                None => None,
                Some(b'[') => {
                    let mut j = i + 2;
                    while j < bytes.len() && (0x20..=0x3f).contains(&bytes[j]) {
                        j += 1;
                    }
                    match bytes.get(j) {
                        None => None,
                        // A real final byte ends the sequence.
                        Some(b) if (0x40..=0x7e).contains(b) => Some(j + 1),
                        // Anything else (a multibyte character, a control
                        // byte) means this was never a sequence: the ESC is
                        // plain text and scanning resumes after it.
                        Some(_) => Some(i + 1),
                    }
                }
                Some(b']') => {
                    let mut j = i + 2;
                    let mut found = None;
                    while j < bytes.len() {
                        if bytes[j] == 0x07 {
                            found = Some(j + 1);
                            break;
                        }
                        if bytes[j] == 0x1b && bytes.get(j + 1) == Some(&b'\\') {
                            found = Some(j + 2);
                            break;
                        }
                        // A terminal ends an OSC at a line break too.
                        if bytes[j] == b'\r' || bytes[j] == b'\n' {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    found
                }
                Some(b) if (0x20..0x7f).contains(b) => Some(i + 2),
                // ESC then a multibyte character or a control byte: a lone ESC.
                Some(_) => Some(i + 1),
            };
            match end {
                Some(end) if end <= bytes.len() => {
                    let seq = &input[i..end];
                    let is_cup = seq.starts_with("\x1b[") && (seq.ends_with('H') || seq.ends_with('f'));
                    if seq.starts_with("\x1b[") && !seq.starts_with("\x1b[?") {
                        self.col = csi_column(seq, self.col);
                        if is_cup {
                            self.drop_dup = None;
                        }
                    }
                    if let Some(on) = alt_screen_switch(seq) {
                        self.alt_screen = on;
                        self.row = None;
                        self.row_has_text = false;
                    }
                    // RIS (ESC c) and DECSTR (CSI ! p) reset the terminal,
                    // the alternate screen with it.
                    if seq == "\x1bc" || seq == "\x1b[!p" {
                        self.alt_screen = false;
                        self.row = None;
                        self.row_has_text = false;
                    }
                    if is_cup && !self.alt_screen {
                        let target = seq[2..seq.len() - 1]
                            .split(';')
                            .next()
                            .map(|r| if r.is_empty() { Some(1) } else { r.parse::<u32>().ok() })
                            .unwrap_or(Some(1));
                        let same_row = target.is_some() && target == self.row;
                        if same_row {
                            out.push_str(seq);
                        } else if self.row_has_text {
                            out.push('\n');
                            self.row_has_text = false;
                        }
                        self.row = target;
                    } else {
                        if !self.alt_screen
                            && seq.starts_with("\x1b[")
                            && matches!(seq.as_bytes()[seq.len() - 1], b'A' | b'B' | b'E' | b'F' | b'd' | b'J')
                        {
                            self.row = None;
                        }
                        out.push_str(seq);
                    }
                    i = end;
                }
                _ => {
                    // Incomplete: keep it for the next read, unless it has
                    // grown too long to be an escape sequence at all. Then
                    // the ESC alone is text and scanning goes on after it, so
                    // a cursor jump later in the read still counts.
                    if bytes.len() - i <= MAX_ESCAPE_CARRY {
                        self.carry = input[i..].to_string();
                        break;
                    }
                    out.push('\x1b');
                    i += 1;
                }
            }
        }
        out
    }

    /// Whatever is still held when the stream ends.
    pub(crate) fn finish(&mut self) -> String {
        let mut out = self.flush_held();
        out.push_str(&std::mem::take(&mut self.carry));
        out
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

pub(crate) fn clone_reader(pair: &PtyPair) -> Result<Box<dyn Read + Send>, String> {
    pair.master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {e}"))
}

/// Text only: markers are dropped. For callers that just want the output.
pub(crate) fn spawn_reader<R, F>(reader: R, on_text: F)
where
    R: Read + Send + 'static,
    F: Fn(String) + Send + 'static,
{
    spawn_reader_with_exit(
        reader,
        Arc::new(AtomicU16::new(PTY_COLS)),
        move |event| {
            if let ReaderEvent::Text(text) = event {
                on_text(text);
            }
        },
        || {},
    );
}

/// Reads the pty on a thread of its own and hands on text and markers in
/// stream order. Calls `on_exit` once the stream ends (EOF or a read error),
/// which for a PTY means the shell is gone. `width` is the console width in
/// columns; it follows resizes.
pub(crate) fn spawn_reader_with_exit<R, F, E>(mut reader: R, width: Arc<AtomicU16>, on_event: F, on_exit: E)
where
    R: Read + Send + 'static,
    F: Fn(ReaderEvent) + Send + 'static,
    E: FnOnce() + Send + 'static,
{
    std::thread::spawn(move || {
        // The read blocks, and a CR/LF held back by the display rewrite has to
        // be released when nothing follows it, so the read gets a thread of its
        // own and this one waits on it with a short timeout.
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        // A panic anywhere in here (a bug in the display rewrite or a
        // consumer) must not leave the session "running" forever: on_exit
        // always runs.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Bytes of a multibyte character that a read cut in half.
            let mut pending: Vec<u8> = Vec::new();
            let mut scanner = MarkerScanner::default();
            let mut rows = RowNormalizer::with_width(width.load(Ordering::Relaxed));
            // The display rewrite is only cosmetic: if it ever panics, keep
            // reading and pass the text on as it came.
            let display = |rows: &mut RowNormalizer, text: String| -> String {
                if !cfg!(windows) {
                    return text;
                }
                let raw = text.clone();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rows.push(&raw))).unwrap_or_else(|_| {
                    *rows = RowNormalizer::with_width(width.load(Ordering::Relaxed));
                    raw
                })
            };
            // A marker is delivered once the stream has been quiet for
            // MARKER_SETTLE, after any text that arrives in the meantime.
            // ConPTY passes an OSC through at once but paints text on its own
            // schedule, so text the shell wrote before its prompt can still be
            // on its way when the marker arrives (seen at boot, where the echo
            // of the bootstrap came after the first marker). Waiting for quiet
            // puts the output first, as the shell wrote it.
            let mut held_marker: Option<Marker> = None;
            let mut idle_sent = true;
            let deliver = |rows: &mut RowNormalizer, marker: Marker| {
                // Output ends here: release a held break, and a prompt means
                // no full-screen program is on the screen any more.
                let mut shown = rows.flush_held();
                if marker.kind == MarkerKind::Prompt {
                    shown.push_str(&rows.end_alt_screen());
                }
                if !shown.is_empty() {
                    on_event(ReaderEvent::Text(shown));
                }
                on_event(ReaderEvent::Marker(marker));
            };
            loop {
                let wait = if held_marker.is_some() { MARKER_SETTLE } else { HELD_BREAK_FLUSH };
                match rx.recv_timeout(wait) {
                    Ok(bytes) => {
                        idle_sent = false;
                        let text = decode_utf8_stream(&mut pending, &bytes);
                        rows.set_width(width.load(Ordering::Relaxed));
                        for event in scanner.push(&text) {
                            match event {
                                ReaderEvent::Text(t) => {
                                    let shown = display(&mut rows, t);
                                    if !shown.is_empty() {
                                        on_event(ReaderEvent::Text(shown));
                                    }
                                }
                                ReaderEvent::Marker(marker) => {
                                    if let Some(earlier) = held_marker.replace(marker) {
                                        deliver(&mut rows, earlier);
                                    }
                                }
                                ReaderEvent::Idle => {}
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(marker) = held_marker.take() {
                            deliver(&mut rows, marker);
                        } else if cfg!(windows) {
                            let text = rows.flush_held();
                            if !text.is_empty() {
                                on_event(ReaderEvent::Text(text));
                            }
                        }
                        if held_marker.is_none() && !idle_sent && wait == HELD_BREAK_FLUSH {
                            idle_sent = true;
                            on_event(ReaderEvent::Idle);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let mut rest = String::new();
            if !pending.is_empty() {
                let tail = String::from_utf8_lossy(&pending).into_owned();
                for event in scanner.push(&tail) {
                    if let ReaderEvent::Text(t) = event {
                        rest.push_str(&display(&mut rows, t));
                    }
                }
            }
            let held = scanner.finish();
            if !held.is_empty() {
                rest.push_str(&display(&mut rows, held));
            }
            rest.push_str(&rows.finish());
            if !rest.is_empty() {
                on_event(ReaderEvent::Text(rest));
            }
            if let Some(marker) = held_marker.take() {
                deliver(&mut rows, marker);
            }
        }));
        on_exit();
    });
}

/// How long a marker waits for text that ConPTY has yet to paint (see the
/// reader). Short enough not to be noticed at the end of a command, longer than
/// a paint interval.
const MARKER_SETTLE: std::time::Duration = std::time::Duration::from_millis(40);

/// How long a CR/LF held back by the display rewrite waits for the next read.
/// Long enough for a loaded machine or a split write between ConPTY's CR LF and
/// the cursor jump that follows it, short enough that the last line of output
/// is not noticeably late. A wrong guess only splits a long line in two for
/// display: markers do not depend on it.
const HELD_BREAK_FLUSH: std::time::Duration = std::time::Duration::from_millis(250);

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

    fn decode_powershell_command(encoded: &str) -> String {
        const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut bits = 0u32;
        let mut n = 0;
        let mut bytes = Vec::new();
        for c in encoded.chars().filter(|&c| c != '=') {
            bits = (bits << 6) | ALPHABET.find(c).expect("base64 character") as u32;
            n += 6;
            if n >= 8 {
                n -= 8;
                bytes.push((bits >> n) as u8);
                bits &= (1 << n) - 1;
            }
        }
        let units: Vec<u16> = bytes.chunks(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        String::from_utf16(&units).expect("UTF-16")
    }

    #[test]
    fn encoded_command_is_base64_of_utf16le() {
        // Known values: what `[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes(...))` gives.
        assert_eq!(encode_powershell_command("Hi"), "SABpAA==");
        assert_eq!(encode_powershell_command("dir"), "ZABpAHIA");
        assert_eq!(encode_powershell_command(""), "");
        let text = "Write-Host 'é ✓ 𝄞'; $x = 1";
        assert_eq!(decode_powershell_command(&encode_powershell_command(text)), text);
    }

    #[test]
    fn powershell_starts_with_its_bootstrap_and_nothing_is_typed() {
        for shell in ["pwsh", "pwsh.exe", "powershell.exe", r"C:\Program Files\PowerShell\7\pwsh.exe"] {
            let args = launch_args(shell, NONCE);
            assert_eq!(&args[..3], ["-NoLogo", "-NoExit", "-EncodedCommand"], "{shell}");
            let script = decode_powershell_command(&args[3]);
            let typed = bootstrap_prompt(shell, NONCE).unwrap();
            assert_eq!(script, typed.strip_suffix(ENTER).unwrap(), "{shell}: the encoded script is the bootstrap");
            assert!(script.contains("function global:prompt"), "{shell}");
            assert!(script.contains(NONCE), "{shell}");
            assert!(!script.ends_with(ENTER), "{shell}: no Enter in an argument");
        }
    }

    #[test]
    fn other_shells_keep_the_typed_bootstrap() {
        for shell in ["cmd.exe", "/bin/bash", "/usr/bin/zsh", "fish"] {
            assert!(launch_args(shell, NONCE).is_empty(), "{shell}");
        }
    }

    #[test]
    fn a_session_never_defaults_to_the_windows_folder() {
        use std::path::PathBuf;
        let home = Some(PathBuf::from(r"C:\Users\someone"));
        let win = Some(PathBuf::from(r"C:\WINDOWS"));
        let pick = |dir: &str| default_session_cwd(Some(PathBuf::from(dir)), home.clone(), win.clone());
        // A packaged app launched from Start runs in System32: start at home.
        assert_eq!(pick(r"C:\Windows\System32"), home);
        assert_eq!(pick(r"C:\WINDOWS\system32"), home);
        assert_eq!(pick(r"C:\Windows\SysWOW64"), home);
        assert_eq!(pick(r"C:\Windows"), home);
        // Anything else is kept, including a folder that only starts with the same letters.
        assert_eq!(pick(r"D:\work\api"), Some(PathBuf::from(r"D:\work\api")));
        assert_eq!(pick(r"C:\WindowsApps\tool"), Some(PathBuf::from(r"C:\WindowsApps\tool")));
        // An unreadable process folder falls back to home.
        assert_eq!(default_session_cwd(None, home.clone(), win.clone()), home);
        // No home known: keep what there is rather than fail.
        assert_eq!(
            default_session_cwd(Some(PathBuf::from(r"C:\Windows\System32")), None, win.clone()),
            Some(PathBuf::from(r"C:\Windows\System32"))
        );
        // No Windows folder (Unix): the process folder is used as it is.
        assert_eq!(
            default_session_cwd(Some(PathBuf::from("/srv/app")), Some(PathBuf::from("/home/u")), None),
            Some(PathBuf::from("/srv/app"))
        );
    }

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
    fn unsupported_shells_have_no_bootstrap() {
        assert!(bootstrap_prompt("/bin/sh", NONCE).is_none());
        assert!(bootstrap_prompt("dash", NONCE).is_none());
        assert!(bootstrap_prompt("fish", NONCE).is_none());
    }

    #[test]
    fn cmd_launch_does_not_enable_delayed_expansion() {
        let cmd = prepare_shell_command("C:\\Windows\\System32\\cmd.exe", Some("/work"), &[]);
        assert!(!cmd.get_argv().iter().any(|arg| arg == "/v:on"));
        assert_eq!(cmd.get_cwd().map(|d| d.to_string_lossy().to_string()).as_deref(), Some("/work"));

        let bash = prepare_shell_command("/bin/bash", None, &[]);
        assert!(!bash.get_argv().iter().any(|arg| arg == "/v:on"));
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
    fn marker_nonce_is_32_hex_and_default_shell_is_nonempty() {
        let nonce = new_marker_nonce();
        assert_eq!(nonce.len(), 32);
        assert!(nonce.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!default_shell().is_empty());
    }

    #[test]
    fn write_raw_succeeds_against_sink() {
        let handle: PtyHandle = Arc::new(Mutex::new(
            Box::new(std::io::sink()) as Box<dyn Write + Send>,
        ));
        write_raw(&handle, "echo hi").unwrap();
        write_raw(&handle, "xyz").unwrap();
    }

    #[test]
    fn write_paths_report_write_flush_and_lock_errors() {
        let fail_write: PtyHandle = Arc::new(Mutex::new(Box::new(FailWrite) as Box<dyn Write + Send>));
        let write_err = write_raw(&fail_write, "echo hi").unwrap_err();
        assert!(write_err.contains("Write error"), "{write_err}");
        let write_err = write_raw(&fail_write, "xyz").unwrap_err();
        assert!(write_err.contains("Write error"), "{write_err}");

        let fail_flush: PtyHandle =
            Arc::new(Mutex::new(Box::new(FailFlush) as Box<dyn Write + Send>));
        let flush_err = write_raw(&fail_flush, "echo hi").unwrap_err();
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
        let lock_err = write_raw(&handle, "echo hi").unwrap_err();
        assert!(lock_err.contains("Lock error"), "{lock_err}");
        let lock_err = write_raw(&handle, "xyz").unwrap_err();
        assert!(lock_err.contains("Lock error"), "{lock_err}");
    }

    #[test]
    fn a_missing_or_dumb_term_gets_xterm_and_a_usable_one_is_kept() {
        use std::ffi::OsStr;
        assert_eq!(default_term(None), Some(("xterm-256color", "truecolor")));
        assert_eq!(default_term(Some(OsStr::new(""))), Some(("xterm-256color", "truecolor")));
        assert_eq!(default_term(Some(OsStr::new("dumb"))), Some(("xterm-256color", "truecolor")));
        assert_eq!(default_term(Some(OsStr::new("xterm-kitty"))), None);
        assert_eq!(default_term(Some(OsStr::new("screen"))), None);
    }

    #[test]
    fn the_zsh_new_user_guard_applies_only_when_the_wizard_would_run() {
        let home = std::env::temp_dir().join(format!("cu-zsh-home-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&home).unwrap();
        // No startup files, no ZDOTDIR: guarded, with a .zshenv that unsets ZDOTDIR.
        let dir = zsh_newuser_guard_dir(Some(&home), None).expect("a guard directory");
        let zshenv = std::fs::read_to_string(dir.join(".zshenv")).unwrap();
        assert!(zshenv.starts_with("unset ZDOTDIR\n"), "{zshenv}");
        assert!(zshenv.contains("skip_global_compinit=1\n"), "{zshenv}");
        assert!(zshenv.contains("rm -rf"), "{zshenv}");
        let _ = std::fs::remove_dir_all(&dir);
        // The user set ZDOTDIR: left alone.
        assert!(zsh_newuser_guard_dir(Some(&home), Some(std::path::Path::new("/somewhere"))).is_none());
        // An empty ZDOTDIR is no ZDOTDIR.
        let dir = zsh_newuser_guard_dir(Some(&home), Some(std::path::Path::new(""))).expect("guard");
        let _ = std::fs::remove_dir_all(dir);
        // Any one startup file means no wizard: left alone, and nothing is created in HOME.
        for name in [".zshenv", ".zprofile", ".zshrc", ".zlogin"] {
            std::fs::write(home.join(name), "").unwrap();
            assert!(zsh_newuser_guard_dir(Some(&home), None).is_none(), "{name}");
            std::fs::remove_file(home.join(name)).unwrap();
        }
        assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
        assert!(zsh_newuser_guard_dir(None, None).is_none());
        let _ = std::fs::remove_dir_all(&home);
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

    #[test]
    fn write_raw_adds_nothing_to_the_users_keystrokes() {
        #[derive(Clone)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let handle: PtyHandle = Arc::new(Mutex::new(Box::new(Capture(bytes.clone())) as Box<dyn Write + Send>));
        // The line ending of an executed command comes from
        // command_line_for_shell (asserted above); write_raw adds none.
        write_raw(&handle, "echo hi").unwrap();
        assert_eq!(bytes.lock().unwrap().as_slice(), b"echo hi");
        bytes.lock().unwrap().clear();
        // The user's keystrokes (already encoded by xterm) pass through untouched.
        write_raw(&handle, "ab\n\r\x1b[A").unwrap();
        assert_eq!(bytes.lock().unwrap().as_slice(), b"ab\n\r\x1b[A");
    }

    #[test]
    fn row_normalizer_turns_cursor_positioning_into_line_breaks() {
        let mut rows = RowNormalizer::default();
        // ConPTY: output, then the marker on the next row with no CR LF.
        assert_eq!(
            rows.push("probe-ok\x1b[?25l\x1b[15;1Hrow-with-text|n|C:\\w|0\x1b[16;1HC:\\w>"),
            "probe-ok\x1b[?25l\nrow-with-text|n|C:\\w|0\nC:\\w>"
        );
        // A move on an empty row, or the screen clear at start-up, is not a break.
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push("\x1b[2J\x1b[m\x1b[Hhello\r\n\x1b[4;1H"), "\x1b[2J\x1b[mhello\r\n");
        // Moving within the row the cursor is on is an in-line redraw: kept.
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push("\x1b[5;1Hab\x1b[5;3Hcd\x1b[6;1Hef"), "ab\x1b[5;3Hcd\nef");
        // Other escapes pass through byte for byte.
        let mut rows = RowNormalizer::default();
        let text = "\x1b[93mred\x1b[0m \x1b]0;title\x07done";
        assert_eq!(rows.push(text), text);
    }

    #[test]
    fn a_conpty_soft_wrap_is_not_a_line_break() {
        // Observed from ConPTY at the bottom of a 120-column screen: the row is
        // filled, then CR LF, then a move back to column 120 that repaints the
        // last character and goes on.
        let row = "c".repeat(118) + "ab";
        let wrapped = format!("\n{row}\r\n\x1b[29;120Hb-and-more|0\r\n");
        let expect = format!("\n{row}-and-more|0\r\n");
        let mut rows = RowNormalizer::default();
        let mut out = rows.push(&wrapped);
        out.push_str(&rows.finish());
        assert_eq!(out, expect);
        // Whatever the read boundaries, and with colour escapes in the break.
        let wrapped = format!("\n{row}\x1b[m\r\n\x1b[33m\x1b[29;120Hb-and-more|0\r\n");
        let expect = format!("\n{row}\x1b[m\x1b[33m-and-more|0\r\n");
        for (cut, _) in wrapped.char_indices() {
            let mut rows = RowNormalizer::default();
            let mut out = rows.push(&wrapped[..cut]);
            out.push_str(&rows.push(&wrapped[cut..]));
            out.push_str(&rows.finish());
            assert_eq!(out, expect, "cut at {cut}");
        }
        // A line three rows long wraps twice, and the second wrap is found
        // because the first one showed the width.
        let mid = "m".repeat(118) + "pq";
        let wrapped = format!("
{row}
[29;120Hb{mid}
[29;120Hq-end|0
");
        let mut rows = RowNormalizer::default();
        let mut out = rows.push(&wrapped);
        out.push_str(&rows.finish());
        assert_eq!(out, format!("
{row}{mid}-end|0
"));
        // The same at another width: a 40-column row.
        let mut rows = RowNormalizer::default();
        let line = "x".repeat(39) + "y";
        let mut out = rows.push(&format!("{line}\r\n\x1b[9;40Hyzzz\r\nnext"));
        out.push_str(&rows.finish());
        assert_eq!(out, format!("{line}zzz\r\nnext"));
    }

    #[test]
    fn a_real_line_break_on_a_wide_row_is_kept() {
        // A break that does not end a row filled to the column the next jump
        // names is a break, and a held break is released when nothing follows.
        let line = "w".repeat(30);
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push(&format!("{line}\r\n\x1b[9;1Hnext")), format!("{line}\r\nnext"));
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push(&format!("{line}\r\n\x1b[9;31Hnext")), format!("{line}\r\nnext"));
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push(&format!("{line}\r\n")), line);
        assert_eq!(rows.flush_held(), "\r\n");
        assert_eq!(rows.flush_held(), "");
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push(&format!("{line}\r\nplain")), format!("{line}\r\nplain"));
    }

    #[test]
    fn row_normalizer_holds_an_escape_cut_by_a_read() {
        let full = "ab\x1b[12;1Hcd";
        for split in 1..full.len() {
            let mut rows = RowNormalizer::default();
            let mut out = rows.push(&full[..split]);
            out.push_str(&rows.push(&full[split..]));
            out.push_str(&rows.finish());
            assert_eq!(out, "ab\ncd", "split at {split}");
        }
        // A sequence that never ends is released at the end of the stream.
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push("x\x1b[1"), "x");
        assert_eq!(rows.finish(), "\x1b[1");
    }

    #[test]
    fn row_normalizer_never_slices_inside_a_character() {
        // ESC, or ESC [, followed by a multibyte character used to panic
        // (`byte index 2 is not a char boundary`) and kill the reader thread.
        for text in [
            "\x1b\u{e9}",
            "a\x1b\u{e9}x",
            "\x1b[\u{e9}x",
            "\x1b[\u{FFFD}",
            "\x1b[1;\u{65e5}",
            "\x1b]0;\u{e9}\x07\u{e9}",
            "\x1b\u{1F600}\x1b[\u{1F600}",
            "\x1b[?\u{e9}h",
        ] {
            let mut rows = RowNormalizer::default();
            let mut out = rows.push(text);
            out.push_str(&rows.finish());
            // Nothing that is not an escape is dropped.
            for ch in text.chars().filter(|c| !c.is_ascii()) {
                assert!(out.contains(ch), "{text:?} lost {ch:?}: {out:?}");
            }
        }
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push("a\x1b\u{e9}b"), "a\x1b\u{e9}b");
    }

    #[test]
    fn row_normalizer_never_panics_and_never_drops_text_for_random_input() {
        let pool: Vec<char> = "\x1b[]\\;?!0123456789HfmKhlABJcp\x07\r\n abc\u{e9}\u{65e5}\u{1F600}\u{FFFD}_|1049"
            .chars()
            .collect();
        let mut seed = 0xD1B54A32D192ED03u64;
        for round in 0..20000 {
            let mut text = String::new();
            for _ in 0..(round % 40) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                text.push(pool[(seed % pool.len() as u64) as usize]);
            }
            // Whole, and cut at every character boundary.
            let mut rows = RowNormalizer::default();
            let whole = {
                let mut o = rows.push(&text);
                o.push_str(&rows.finish());
                o
            };
            let boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
            for cut in boundaries {
                let mut rows = RowNormalizer::default();
                let mut out = rows.push(&text[..cut]);
                out.push_str(&rows.push(&text[cut..]));
                out.push_str(&rows.finish());
                let _ = out;
            }
            // Every non-ASCII character of the input is still there: only
            // escape sequences (ASCII) are ever rewritten.
            for ch in text.chars().filter(|c| !c.is_ascii()) {
                assert!(whole.contains(ch), "{text:?} -> {whole:?} lost {ch:?}");
            }
        }
    }

    #[test]
    fn an_oversized_escape_does_not_hold_back_later_output_or_markers() {
        let mut rows = RowNormalizer::default();
        let long = format!("\x1b]0;{}", "t".repeat(400));
        let text = format!("{long}\x1b[5;1Hone\x1b[6;1Htwo");
        let out = rows.push(&text);
        // The unterminated title is text, scanning went on after it, and the
        // cursor jump later in the same read is still turned into a line break.
        assert!(out.contains("one\ntwo"), "{out:?}");
        // An OSC ends at a line break as it does in a terminal.
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push("\x1b]0;title\nline\n"), "\x1b]0;title\nline\n");
    }

    #[test]
    fn find_on_path_returns_the_first_existing_file() {
        let root = std::env::temp_dir().join(format!("commandui-findpath-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("tool.exe"), b"").unwrap();
        std::fs::create_dir_all(a.join("dir.exe")).unwrap();
        let path = std::env::join_paths([&a, &b]).unwrap();
        assert_eq!(find_on_path("tool.exe", Some(&path)), Some(b.join("tool.exe")));
        assert_eq!(find_on_path("dir.exe", Some(&path)), None, "a directory is not a program");
        assert_eq!(find_on_path("missing.exe", Some(&path)), None);
        assert_eq!(find_on_path("tool.exe", None), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn alternate_screen_output_passes_through_untouched() {
        let screen = "\x1b[1;1Hone\x1b[2;1Htwo\x1b[3;5H\x1b[10;1Hx";
        let mut rows = RowNormalizer::default();
        let input = format!("before\x1b[?1049h{screen}\x1b[?1049lafter\x1b[7;1Hz");
        // Normal screen before, verbatim inside, normal rewriting after.
        assert_eq!(
            rows.push(&input),
            format!("before\x1b[?1049h{screen}\x1b[?1049lafter\nz")
        );
        // Enter and exit each cut across two reads.
        for cut_at in ["\x1b[?10", "\x1b[?1049", "\x1b[?1049h", "\x1b[?1049l", "one\x1b[2;"] {
            let at = input.find(cut_at).unwrap() + cut_at.len();
            let mut rows = RowNormalizer::default();
            let mut out = rows.push(&input[..at]);
            out.push_str(&rows.push(&input[at..]));
            assert_eq!(out, format!("before\x1b[?1049h{screen}\x1b[?1049lafter\nz"), "cut after {cut_at:?}");
        }
        let split = input.find("\x1b[?1049l").unwrap() + 4;
        let mut rows = RowNormalizer::default();
        let mut out = rows.push(&input[..split]);
        out.push_str(&rows.push(&input[split..]));
        assert_eq!(out, format!("before\x1b[?1049h{screen}\x1b[?1049lafter\nz"));
    }

    #[test]
    fn combined_private_modes_switch_the_alternate_screen() {
        assert_eq!(alt_screen_switch("\x1b[?1049;1006h"), Some(true));
        assert_eq!(alt_screen_switch("\x1b[?25;1047l"), Some(false));
        assert_eq!(alt_screen_switch("\x1b[?47h"), Some(true));
        assert_eq!(alt_screen_switch("\x1b[?25h"), None);
        assert_eq!(alt_screen_switch("\x1b[1049h"), None);
        let mut rows = RowNormalizer::default();
        let text = "a\x1b[?1049;1006h\x1b[2;1Hb\x1b[3;1Hc\x1b[?1006;1049l";
        assert_eq!(rows.push(text), text);
    }

    // ---- Markers: the wire format ----

    #[test]
    fn markers_round_trip_through_the_scanner() {
        let osc = marker_osc('P', NONCE, Some(7), Some(true), "/work/a b");
        let mut scanner = MarkerScanner::default();
        let events = scanner.push(&format!("before{osc}after"));
        assert_eq!(
            events,
            vec![
                ReaderEvent::Text("before".into()),
                ReaderEvent::Marker(Marker {
                    kind: MarkerKind::Prompt,
                    nonce: NONCE.into(),
                    exit: Some(7),
                    chord: Some(true),
                    cwd: "/work/a b".into(),
                }),
                ReaderEvent::Text("after".into()),
            ]
        );
        // ST terminates a marker as BEL does (cmd's PROMPT can only write ST).
        let st = format!("{}{}", "\x1b]7733;P;abc;;;C:\\w", "\x1b\\");
        let mut scanner = MarkerScanner::default();
        let events = scanner.push(&st);
        assert!(matches!(&events[..], [ReaderEvent::Marker(m)] if m.cwd == "C:\\w" && m.exit.is_none() && m.chord.is_none()), "{events:?}");
        // The exit marker cmd's tail writes.
        let mut scanner = MarkerScanner::default();
        let events = scanner.push("\x1b]7733;X;abc;7;;\x1b\\");
        assert!(matches!(&events[..], [ReaderEvent::Marker(m)] if m.kind == MarkerKind::Exit && m.exit == Some(7)), "{events:?}");
    }

    #[test]
    fn a_marker_cut_at_any_byte_is_still_one_marker_and_never_text() {
        let osc = marker_osc('P', NONCE, Some(0), None, "/h\u{f6}me/\u{65e5}\u{672c} dir");
        let stream = format!("out1\r\n{osc}out2{osc}");
        let bytes = stream.as_bytes();
        for cut in 1..bytes.len() {
            let mut pending = Vec::new();
            let mut scanner = MarkerScanner::default();
            let mut events = Vec::new();
            for part in [&bytes[..cut], &bytes[cut..]] {
                let text = decode_utf8_stream(&mut pending, part);
                events.extend(scanner.push(&text));
            }
            let text: String = events
                .iter()
                .filter_map(|e| match e {
                    ReaderEvent::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            let markers = events.iter().filter(|e| matches!(e, ReaderEvent::Marker(_))).count();
            assert_eq!(text, "out1\r\nout2", "cut at {cut}");
            assert_eq!(markers, 2, "cut at {cut}");
            assert_eq!(scanner.finish(), "");
        }
        // Byte by byte.
        let mut pending = Vec::new();
        let mut scanner = MarkerScanner::default();
        let mut markers = 0;
        for b in bytes {
            let text = decode_utf8_stream(&mut pending, &[*b]);
            for event in scanner.push(&text) {
                if let ReaderEvent::Marker(m) = event {
                    assert_eq!(m.cwd, "/h\u{f6}me/\u{65e5}\u{672c} dir");
                    markers += 1;
                }
            }
        }
        assert_eq!(markers, 2);
    }

    #[test]
    fn a_300_character_cwd_is_one_marker_whatever_the_console_width() {
        // The marker is not text on a row: nothing about the width matters.
        let cwd = format!("C:\\{}", "deep\\".repeat(60));
        assert!(cwd.len() > 300);
        let osc = marker_osc('P', NONCE, Some(0), Some(true), &cwd);
        let mut scanner = MarkerScanner::default();
        let events = scanner.push(&osc);
        assert!(matches!(&events[..], [ReaderEvent::Marker(m)] if m.cwd == cwd), "{events:?}");
    }

    #[test]
    fn the_cwd_field_cannot_carry_a_terminator_a_separator_or_a_line_break() {
        let nasty = "/a;b%c\x07d\x1be\rf\ng";
        let encoded = encode_cwd(nasty);
        for forbidden in [';', '\x07', '\x1b', '\r', '\n'] {
            assert!(!encoded.contains(forbidden), "{encoded:?} has {forbidden:?}");
        }
        assert_eq!(decode_cwd(&encoded), nasty);
        // A `%` that is not an escape is kept; a lone `%` at the end too.
        assert_eq!(decode_cwd("50%"), "50%");
        assert_eq!(decode_cwd("50%2"), "50%2");
        assert_eq!(decode_cwd("a%zzb"), "a%zzb");
        assert_eq!(decode_cwd("%E6%97%A5"), "\u{65e5}");
        let osc = marker_osc('P', NONCE, Some(0), None, nasty);
        let mut scanner = MarkerScanner::default();
        let events = scanner.push(&osc);
        assert!(matches!(&events[..], [ReaderEvent::Marker(m)] if m.cwd == nasty), "{events:?}");
    }

    #[test]
    fn malformed_markers_are_dropped_and_never_shown() {
        for body in [
            "P;;;;/w",             // no nonce
            "Q;n;0;;/w",           // unknown kind
            "P;n;nope;;/w",        // exit is not an integer
            "P;n;0;;",             // fine: empty cwd is parsed (the service refuses it)
            "P;n",                 // too few fields
        ] {
            let mut scanner = MarkerScanner::default();
            let events = scanner.push(&format!("a\x1b]7733;{body}\x07b"));
            let text: String = events
                .iter()
                .filter_map(|e| match e {
                    ReaderEvent::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(text, "ab", "{body}: {events:?}");
        }
        assert!(parse_marker("P;n;nope;;/w").is_none());
        assert!(parse_marker("P;n;0;;").is_some());
    }

    #[test]
    fn other_escapes_pass_through_and_titles_are_dropped() {
        let mut scanner = MarkerScanner::default();
        let text = "\x1b[93mred\x1b[0m \x1b]8;;http://x\x07link\x1b]8;;\x07 \x1b[?25h\x1bc";
        assert_eq!(scanner.push(text), vec![ReaderEvent::Text(text.to_string())]);
        assert_eq!(scanner.push("a\x1b]0;C:\\windows\\cmd.exe\x07b"), vec![ReaderEvent::Text("ab".into())]);
        assert_eq!(scanner.push("a\x1b]2;t\x1b\\b\x1b]0;x\x07c"), vec![ReaderEvent::Text("abc".into())]);
        // A title with plumbing in it, cut anywhere, is still dropped whole.
        let title = "x\x1b]0;C:\\Windows\\cmd.exe - call set /p=]7733;X;abc;%ERRORLEVEL%\x07y";
        for cut in 1..title.len() {
            if !title.is_char_boundary(cut) {
                continue;
            }
            let mut scanner = MarkerScanner::default();
            let mut all = String::new();
            for part in [&title[..cut], &title[cut..]] {
                for event in scanner.push(part) {
                    if let ReaderEvent::Text(t) = event {
                        all.push_str(&t);
                    }
                }
            }
            assert_eq!(all, "xy", "cut at {cut}");
        }
    }

    #[test]
    fn an_unfinished_osc_is_bounded_and_a_half_marker_at_the_end_is_dropped() {
        // No terminator ever: after MAX_OSC_LEN it is text, scanning goes on,
        // and a marker after it still counts.
        let mut scanner = MarkerScanner::default();
        let junk = format!("\x1b]52;{}", "j".repeat(MAX_OSC_LEN + 10));
        let osc = marker_osc('P', NONCE, Some(0), None, "/w");
        let mut markers = 0;
        for event in scanner.push(&format!("{junk}{osc}")) {
            if matches!(event, ReaderEvent::Marker(_)) {
                markers += 1;
            }
        }
        assert_eq!(markers, 1);
        // The stream ends in the middle of a marker: it is dropped, not shown.
        let mut scanner = MarkerScanner::default();
        assert!(scanner.push("x\x1b]7733;P;abc;0").iter().all(|e| matches!(e, ReaderEvent::Text(t) if t == "x")));
        assert_eq!(scanner.finish(), "");
        let mut scanner = MarkerScanner::default();
        assert_eq!(scanner.push("x\x1b"), vec![ReaderEvent::Text("x".into())]);
        assert_eq!(scanner.finish(), "\x1b");
    }

    #[test]
    fn a_reader_hands_on_markers_in_order_and_shows_no_marker_text() {
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
        let line = format!("out\r\n{}tail", marker_osc('P', NONCE, Some(3), None, "/h\u{f6}me/\u{e9}")).into_bytes();
        let cut = line.iter().position(|b| *b == 0xc3).unwrap() + 1;
        let chunks = vec![line[..cut].to_vec(), line[cut..].to_vec()];
        let (tx, rx) = std::sync::mpsc::channel::<ReaderEvent>();
        let tx = Mutex::new(tx);
        spawn_reader_with_exit(
            Chunks(chunks),
            Arc::new(AtomicU16::new(PTY_COLS)),
            move |event| {
                tx.lock().unwrap().send(event).unwrap();
            },
            || {},
        );
        let mut events = Vec::new();
        while let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            events.push(e);
        }
        let text: String = events
            .iter()
            .filter_map(|e| match e {
                ReaderEvent::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(!text.contains('\u{FFFD}') && !text.contains("7733") && !text.contains('\x1b'), "{text:?}");
        let at_marker = events.iter().position(|e| matches!(e, ReaderEvent::Marker(_))).expect("a marker");
        let before: String = events[..at_marker]
            .iter()
            .filter_map(|e| match e {
                ReaderEvent::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        // The marker is delivered after the text that followed it in the same
        // read (the reader waits for the stream to settle, so output ConPTY
        // paints late is not overtaken by the marker).
        assert!(before.starts_with("out") && before.ends_with("tail"), "{before:?}");
        assert!(matches!(&events[at_marker], ReaderEvent::Marker(m) if m.cwd == "/h\u{f6}me/\u{e9}" && m.exit == Some(3)));
        assert_eq!(at_marker, events.len() - 1, "{events:?}");
    }

    #[test]
    fn a_marker_waits_for_text_that_arrives_within_the_settle_time() {
        struct Timed(Vec<(u64, Vec<u8>)>);
        impl Read for Timed {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                let (delay, next) = self.0.remove(0);
                std::thread::sleep(std::time::Duration::from_millis(delay));
                buf[..next.len()].copy_from_slice(&next);
                Ok(next.len())
            }
        }
        let marker = marker_osc('P', NONCE, Some(0), None, "/w");
        // The marker first, the output it follows 10 ms later (what ConPTY did
        // at boot), then a second marker long after.
        let chunks = vec![(0, marker.clone().into_bytes()), (10, b"late-output\r\n".to_vec())];
        let (tx, rx) = std::sync::mpsc::channel::<ReaderEvent>();
        let tx = Mutex::new(tx);
        spawn_reader_with_exit(
            Timed(chunks),
            Arc::new(AtomicU16::new(PTY_COLS)),
            move |event| {
                tx.lock().unwrap().send(event).unwrap();
            },
            || {},
        );
        let mut events = Vec::new();
        while let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            events.push(e);
        }
        let text: String = events
            .iter()
            .filter_map(|e| match e {
                ReaderEvent::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "late-output\r\n", "{events:?}");
        assert!(matches!(events.last(), Some(ReaderEvent::Marker(_))), "the marker came before the late output: {events:?}");
        assert_eq!(events.iter().filter(|e| matches!(e, ReaderEvent::Marker(_))).count(), 1);
    }

    // ---- Shell hooks ----

    #[test]
    fn every_hook_writes_the_osc_marker_with_nonce_exit_and_cwd() {
        let pwsh = bootstrap_prompt("powershell.exe", NONCE).unwrap();
        assert!(pwsh.contains("]7733;P;"), "{pwsh}");
        assert!(pwsh.contains(NONCE));
        assert!(pwsh.contains("$?"));
        assert!(pwsh.contains("LASTEXITCODE"));
        assert!(pwsh.contains("Get-Location"));
        assert!(pwsh.contains("$__cui_code = 1"));
        assert!(pwsh.contains("if ($__cui_ok) { $__cui_code = 0 }"));
        assert!(pwsh.contains("[Console]::Write("), "the marker is one console write: {pwsh}");
        assert!(!pwsh.contains('~'));

        let cmd = bootstrap_prompt("cmd.exe", NONCE).unwrap();
        assert!(cmd.contains("%^ERRORLEVEL%"));
        assert!(cmd.contains(&format!("prompt $e]7733;P;{NONCE};;;$P$e\\$P$G")), "{cmd}");
        assert!(cmd.contains(&format!("]7733;X;{NONCE};%^ERRORLEVEL%;;")), "{cmd}");

        let bash = bootstrap_prompt("/bin/bash", NONCE).unwrap();
        assert!(bash.contains("$PWD"));
        assert!(bash.contains("ec=$?"));
        assert!(bash.contains(NONCE));
        assert!(bash.contains("]7733;P;%s;%s;%s;%s"), "{bash}");
        assert!(bash.contains("PROMPT_COMMAND=__cui_prompt"));
        assert!(!bash.contains("\\w"));

        let zsh = bootstrap_prompt("/bin/zsh", NONCE).unwrap();
        assert!(zsh.contains("${PWD}"));
        assert!(zsh.contains("$?"));
        assert!(zsh.contains(NONCE));
        assert!(zsh.contains("]7733;P;"), "{zsh}");
        assert!(!zsh.contains("%~"));
    }

    #[test]
    fn no_hook_prints_marker_text_or_a_line_break() {
        // The old marker was a visible `__COMMANDUI_PROMPT__|...` row preceded
        // by a newline. The hooks write only the OSC.
        for shell in ["powershell.exe", "pwsh", "cmd.exe", "/bin/bash", "/bin/zsh"] {
            let boot = bootstrap_prompt(shell, NONCE).unwrap();
            assert!(!boot.contains("COMMANDUI"), "{shell}: {boot}");
            assert!(!boot.contains("print -r -- \"\""), "{shell}");
        }
        assert!(!bootstrap_prompt("/bin/bash", NONCE).unwrap().contains("printf \"\\n"));
    }

    #[test]
    fn hooks_encode_the_cwd_so_it_cannot_end_the_sequence() {
        for (shell, parts) in [
            ("/bin/bash", vec!["\\%/%25", ";/%3B", "$'\\e'/%1B", "$'\\a'/%07", "$'\\n'/%0A", "$'\\r'/%0D"]),
            ("/bin/zsh", vec!["\\%/%25", ";/%3B", "$'\\e'/%1B", "$'\\a'/%07", "$'\\n'/%0A", "$'\\r'/%0D"]),
            // Bytes, not characters: [Console]::Write would turn a non-ASCII
            // character into `?` in the console's code page.
            ("pwsh", vec!["UTF8.GetBytes((Get-Location).Path)", "$_ -lt 32 -or $_ -gt 126 -or $_ -eq 37 -or $_ -eq 59", "'%{0:X2}' -f $_"]),
        ] {
            let boot = bootstrap_prompt(shell, NONCE).unwrap();
            for part in parts {
                assert!(boot.contains(part), "{shell} lacks {part}: {boot}");
            }
        }
    }

    #[test]
    fn the_clear_chord_is_bound_only_when_free_and_read_back() {
        let ps = bootstrap_prompt("pwsh", NONCE).unwrap();
        // Vi insert, vi command and the other modes each get a handler.
        assert!(ps.contains("-ViMode Insert -Chord 'Ctrl+]' -Function RevertLine"), "{ps}");
        assert!(ps.contains("-ViMode Command -Chord 'Ctrl+]' -ScriptBlock"), "{ps}");
        assert!(ps.contains("ViInsertMode()"), "{ps}");
        // The non-Vi branch, with its own `else`, is not a substring of the
        // Vi insert binding above.
        assert!(ps.contains("} else { Set-PSReadLineKeyHandler -Chord 'Ctrl+]' -Function RevertLine }"), "{ps}");
        assert_eq!(ps.matches("Set-PSReadLineKeyHandler").count(), 3, "{ps}");
        // Only when unbound or still a default, and only when the edit mode
        // changed since the last binding (not at every prompt).
        assert!(ps.contains("-notin 'GotoBrace','CharacterSearch','RevertLine'"), "{ps}");
        assert!(ps.contains("if ($__cui_m -ne $global:__cui_mode)"), "{ps}");
        // Read back, and the marker reports it.
        assert!(ps.contains("[string]$_.Function -eq 'RevertLine'"), "{ps}");
        assert!(ps.find("Set-PSReadLineKeyHandler").unwrap() < ps.find("]7733;P;").unwrap());

        let bash = bootstrap_prompt("/bin/bash", NONCE).unwrap();
        for map in ["emacs", "vi-insert", "vi-command"] {
            assert!(bash.contains(&format!("bind -m {map} ")), "{map}: {bash}");
            assert!(bash.contains(&format!("__cui_free {map}")), "{map}: {bash}");
        }
        assert!(bash.contains("kill-whole-line"));
        // Unbound or readline's own default only (character-search in emacs,
        // self-insert in vi insert); verified before it is relied on.
        assert!(bash.contains("[ \"$l\" = '\"\\C-]\": character-search' ]"), "{bash}");
        assert!(bash.contains("[ \"$l\" = '\"\\C-]\": self-insert' ]"), "{bash}");
        assert!(bash.contains("then __cui_chord=1; fi"), "{bash}");
        assert_eq!(CLEAR_CHORD, "\x1d");
    }

    #[test]
    fn a_shell_that_says_the_chord_is_unbound_gets_the_fallback_clear() {
        assert_eq!(clear_input_line("/bin/bash", true), "\x1d");
        assert_eq!(clear_input_line("/bin/bash", false), "\x05\x15");
        assert_eq!(clear_input_line("pwsh.exe", true), "\x1d\x1b[1;5F\x1b[1;5H");
        assert_eq!(clear_input_line("pwsh.exe", false), "\x1b[1;5F\x1b[1;5H");
        assert_eq!(clear_input_line("zsh", true), "\x05\x15");
        assert_eq!(clear_input_line("cmd.exe", true), "\x1b[1;5F\x1b[1;5H");
        assert_eq!(clear_input_line("fish", true), "");
    }

    #[test]
    fn resync_clears_the_line_before_its_enter() {
        // A half-typed line must not be submitted by the resync's Enter.
        assert_eq!(resync_input("bash", true), "\x1d\r");
        assert_eq!(resync_input("bash", false), "\x05\x15\r");
        assert_eq!(resync_input("pwsh", true), "\x1d\x1b[1;5F\x1b[1;5H\r");
        assert_eq!(resync_input("zsh", true), "\x05\x15\r");
        assert_eq!(resync_input("cmd.exe", true), "\x1b[1;5F\x1b[1;5H\r");
    }

    // ---- cmd command lines ----

    #[test]
    fn cmd_command_line_chains_the_exit_tail_on_the_same_line() {
        let nasty = "echo %CD% & del /q *";
        let (line, tail) = command_line_for_shell("cmd.exe", nasty, true);
        assert_eq!(tail, CmdTail::Chained);
        // One line, one Enter: a program that flushes the console input
        // buffer (pause, choice, set /p) has no typed-ahead tail to eat.
        assert_eq!(line, format!("\x1b[1;5F\x1b[1;5H%__cuz% & {nasty} & %__cui%\r"));
        assert_eq!(line.matches('\r').count(), 1);
        assert!(!line.contains('\n'), "a Windows ConPTY submits on CR; LF is Ctrl+J");
        // The tail is in the bootstrap's variable, with the nonce, and the
        // ERRORLEVEL reset is the other one (`(call )` sets it to 0).
        let boot = bootstrap_prompt("cmd.exe", NONCE).unwrap();
        assert!(boot.contains(&format!("\"__cui=call <nul set /p=%__cue%]7733;X;{NONCE};%^ERRORLEVEL%;;%__cue%\\\"\r")), "{boot}");
        assert!(boot.contains("\r@set \"__cuz=(call )\"\r"), "{boot}");
        assert_eq!(command_line_for_shell("bash", "ls", true).0, "\x1dls\r");
        assert_eq!(cmd_probe_line(true), "\x1b[1;5F\x1b[1;5H%__cui%\r");
    }

    #[test]
    fn cmd_commands_that_could_swallow_or_break_the_tail_are_written_alone() {
        for command in [
            "echo hi & rem",
            "echo hi & :: comment",
            ":: comment",
            "echo \"unbalanced",
            "echo hi ^",
            "(echo a",
            "echo a)",
            // Trailing operators: `echo a & & tail` does not parse.
            "echo a &",
            "echo a |",
            "echo a >",
            "echo a <",
            "echo a &&",
            "echo a ||",
            // if / for / rem as the first word of a segment.
            "if exist x echo y",
            "for /l %i in (1,1,3) do @echo %i",
            "rem hello",
            "@rem hello",
            "echo a & if x==x echo b",
            "echo a | for %i in (1) do echo %i",
            "(if 1==1 echo y)",
            "IF EXIST x echo y",
        ] {
            let (line, tail) = command_line_for_shell("cmd.exe", command, true);
            assert_eq!(tail, CmdTail::None, "{command}");
            assert_eq!(line, format!("\x1b[1;5F\x1b[1;5H%__cuz% & {command}\r"), "{command}");
        }
        // The same words as arguments, in quotes, or inside another word chain.
        for command in [
            "echo for you",
            "echo if only",
            "git commit -m \"fix for bug\"",
            "findstr if x",
            "echo rem",
            "dir /b",
            "cd /d \"C:\\a b\"",
            "echo (a) & echo b",
            "git status",
            "echo format & echo information",
            // `::` is a comment only at the start of a command.
            "echo hi :: not a comment",
        ] {
            let (line, tail) = command_line_for_shell("cmd.exe", command, true);
            assert_eq!(tail, CmdTail::Chained, "{command}");
            assert!(line.ends_with(" & %__cui%\r"), "{command}: {line:?}");
        }
    }

    #[test]
    fn command_line_clears_pending_input_per_shell_family() {
        assert_eq!(command_line_for_shell("zsh", "ls", true).0, "\x05\x15ls\r");
        assert_eq!(command_line_for_shell("pwsh.exe", "ls", true).0, "\x1d\x1b[1;5F\x1b[1;5Hls\r");
        assert_eq!(command_line_for_shell("powershell.exe", "ls", false).0, "\x1b[1;5F\x1b[1;5Hls\r");
        assert_eq!(command_line_for_shell("/bin/bash", "ls", true).0, "\x1dls\r");
        assert_eq!(command_line_for_shell("/bin/bash", "ls", false).0, "\x05\x15ls\r");
        assert_eq!(
            command_line_for_shell("cmd.exe", "dir", true).0,
            "\x1b[1;5F\x1b[1;5H%__cuz% & dir & %__cui%\r"
        );
    }

    #[test]
    fn cmd_commands_with_bangs_are_written_through_unchanged() {
        for command in ["echo hello!", "git commit -m \"done!\"", "cd hello!world"] {
            let (line, _) = command_line_for_shell("cmd.exe", command, true);
            assert_eq!(line, format!("\x1b[1;5F\x1b[1;5H%__cuz% & {command} & %__cui%\r"), "{line}");
        }
        // The session itself never enables delayed expansion.
        let cmd = prepare_shell_command("cmd.exe", None, &[]);
        assert!(!cmd.get_argv().iter().any(|arg| arg == "/v:on"));
        assert!(!bootstrap_prompt("cmd.exe", NONCE).unwrap().contains("/v:on"));
    }

    #[test]
    fn every_line_runtime_core_builds_is_submitted_with_cr_not_lf() {
        // A Windows ConPTY runs a line on CR (Enter); LF is Ctrl+J there and
        // the bootstrap was typed and never run.
        for shell in ["powershell.exe", "pwsh", "cmd.exe", "/bin/bash", "/bin/zsh"] {
            let boot = bootstrap_prompt(shell, NONCE).unwrap();
            assert!(boot.ends_with('\r'), "{shell}: {boot:?}");
            assert!(!boot.contains('\n'), "{shell}: {boot:?}");
            let (line, _) = command_line_for_shell(shell, "ls", true);
            assert!(line.ends_with('\r'), "{shell}: {line:?}");
            assert!(!line.contains('\n'), "{shell}: {line:?}");
            let resync = resync_input(shell, true);
            assert!(resync.ends_with('\r'), "{shell}: {resync:?}");
            assert!(!resync.contains('\n'), "{shell}: {resync:?}");
        }
        let probe = cmd_probe_line(true);
        assert!(probe.ends_with('\r') && !probe.contains('\n'));
    }

    #[test]
    fn clear_line_is_a_csi_pair_never_a_lone_escape_on_windows_shells() {
        // ESC followed by more bytes in one write is Alt+<key> to a ConPTY:
        // `xyz` + ESC + `echo ok` ran `xyzecho ok`. Ctrl+End / Ctrl+Home as
        // CSI sequences are unambiguous.
        assert_eq!(clear_input_line("cmd.exe", true), "\x1b[1;5F\x1b[1;5H");
        for shell in ["powershell.exe", "pwsh.exe"] {
            assert_eq!(clear_input_line(shell, true), "\x1d\x1b[1;5F\x1b[1;5H", "{shell}");
        }
    }

    // ---- cmd plumbing in the display ----

    #[test]
    fn cmd_plumbing_is_stripped_whatever_the_read_boundaries() {
        let echo = "C:\\w>%__cuz% & dir /b & %__cui%\r\nfile.txt\r\nC:\\w>%__cui%\r\n";
        let clean = "C:\\w>dir /b\r\nfile.txt\r\nC:\\w>\r\n";
        // Cut into two reads at every offset, and into one-byte reads.
        for cut in 0..=echo.len() {
            let mut hold = String::new();
            let mut shown = strip_cmd_plumbing(&mut hold, &echo[..cut], false);
            shown.push_str(&strip_cmd_plumbing(&mut hold, &echo[cut..], false));
            shown.push_str(&strip_cmd_plumbing(&mut hold, "", true));
            assert_eq!(shown, clean, "cut at {cut}");
        }
        let mut hold = String::new();
        let mut shown = String::new();
        for ch in echo.chars() {
            shown.push_str(&strip_cmd_plumbing(&mut hold, &ch.to_string(), false));
        }
        shown.push_str(&strip_cmd_plumbing(&mut hold, "", true));
        assert_eq!(shown, clean);
        // The observed live split: `%__` then `cuz% & dir /b`.
        let mut hold = String::new();
        assert_eq!(strip_cmd_plumbing(&mut hold, "C:\\w>%__", false), "C:\\w>");
        assert_eq!(strip_cmd_plumbing(&mut hold, "cuz% & dir /b", false), "dir /b");
        // Text that merely starts like plumbing is released when it turns out not to be.
        let mut hold = String::new();
        assert_eq!(strip_cmd_plumbing(&mut hold, "50%", false), "50");
        assert_eq!(strip_cmd_plumbing(&mut hold, " done", false), "% done");
        // And at the prompt (flush) nothing is kept back.
        let mut hold = String::new();
        assert_eq!(strip_cmd_plumbing(&mut hold, "a &", true), "a &");
        assert!(hold.is_empty());
    }

    #[test]
    fn the_echo_of_the_cmd_bootstrap_is_stripped_whatever_the_read_boundaries() {
        let boot = bootstrap_cmd("n0nce");
        let lines = cmd_bootstrap_echo(&boot);
        assert_eq!(lines.len(), 4, "{lines:?}");
        let prompt = r"C:\long\cwd>";
        let mut echo = String::new();
        for line in &lines {
            echo.push_str(&format!("{prompt}{line}\r\n"));
        }
        let clean = format!("{prompt}\r\n").repeat(4);
        for cut in 0..=echo.len() {
            let (mut todo, mut hold) = (lines.clone(), String::new());
            let mut shown = strip_cmd_echo(&mut todo, &mut hold, &echo[..cut], false);
            shown.push_str(&strip_cmd_echo(&mut todo, &mut hold, &echo[cut..], false));
            shown.push_str(&strip_cmd_echo(&mut todo, &mut hold, "", true));
            assert_eq!(shown, clean, "cut at {cut}");
        }
        let (mut todo, mut hold) = (lines.clone(), String::new());
        let mut shown = String::new();
        for ch in echo.chars() {
            shown.push_str(&strip_cmd_echo(&mut todo, &mut hold, &ch.to_string(), false));
        }
        shown.push_str(&strip_cmd_echo(&mut todo, &mut hold, "", true));
        assert_eq!(shown, clean);
        assert!(todo.is_empty());
    }

    #[test]
    fn segment_first_words_are_found_outside_quotes_only() {
        assert!(cmd_segment_swallows_tail("if a==a echo y"));
        assert!(cmd_segment_swallows_tail("  FOR %i in (1) do echo %i"));
        assert!(cmd_segment_swallows_tail("echo a&rem x"));
        assert!(cmd_segment_swallows_tail("echo a | if x==x echo"));
        assert!(!cmd_segment_swallows_tail("echo \"a & if b\""));
        assert!(!cmd_segment_swallows_tail("echo if"));
        assert!(!cmd_segment_swallows_tail("iffy"));
        assert!(!cmd_segment_swallows_tail("format c:"));
        assert!(!cmd_segment_swallows_tail("forfiles /p ."));
        assert!(!cmd_segment_swallows_tail("remote x"));
    }

    // ---- Paste accounting ----

    #[test]
    fn submitted_lines_counts_terminators_outside_a_bracketed_paste() {
        assert_eq!(submitted_lines(""), 0);
        assert_eq!(submitted_lines("abc"), 0);
        assert_eq!(submitted_lines("abc\r"), 1);
        assert_eq!(submitted_lines("a\r\nb\r\n"), 2);
        assert_eq!(submitted_lines("a\nb\n"), 2);
        assert_eq!(submitted_lines("sleep 1; echo A\rsleep 6; echo B\r"), 2);
        assert_eq!(submitted_lines("\r\r"), 2);
        // A bracketed paste inserts its line breaks; only the Enter after it runs.
        assert_eq!(submitted_lines("\x1b[200~a\rb\r\x1b[201~"), 0);
        assert_eq!(submitted_lines("\x1b[200~a\rb\x1b[201~\r"), 1);
        assert_eq!(submitted_lines("x\r\x1b[200~a\rb\x1b[201~\r"), 2);
        assert_eq!(submitted_lines("\x1b[200~a\rb"), 0);
        assert_eq!(submitted_lines("\x1b[A\x1b[B"), 0);
    }

    // ---- Display rewrite ----

    #[test]
    fn a_marker_less_prompt_ends_the_alternate_screen_and_starts_its_own_row() {
        // The program was killed: it never sent ?1049l. Its screen text is
        // still on the row when the prompt (and so its marker) arrives.
        let mut rows = RowNormalizer::default();
        let out = rows.push("\x1b[?1049hscreen text");
        assert_eq!(out, "\x1b[?1049hscreen text");
        assert!(rows.alt_screen);
        assert_eq!(rows.end_alt_screen(), "\n");
        assert!(!rows.alt_screen);
        // Back to rewriting cursor jumps.
        assert_eq!(rows.push("next\x1b[9;1Hmore"), "next\nmore");
        // Not in the alternate screen: nothing is added.
        assert_eq!(rows.end_alt_screen(), "");
        // Text that happens to look like the old marker never ends it.
        let mut rows = RowNormalizer::default();
        rows.push("\x1b[?1049h");
        let shown = rows.push("__COMMANDUI_PROMPT__|x|y|0\x1b[5;1Hmore");
        assert!(rows.alt_screen, "{shown:?}");
        assert_eq!(shown, "__COMMANDUI_PROMPT__|x|y|0\x1b[5;1Hmore");
        // RIS and DECSTR also leave it.
        let mut rows = RowNormalizer::default();
        rows.push("\x1b[?1049h");
        assert!(rows.alt_screen);
        rows.push("\x1bc");
        assert!(!rows.alt_screen);
        rows.push("\x1b[?47h");
        assert!(rows.alt_screen);
        rows.push("\x1b[!p");
        assert!(!rows.alt_screen);
    }

    #[test]
    fn a_line_of_several_console_rows_is_one_line_whether_or_not_the_width_is_known() {
        // Two soft wraps in a row, the first with the first row not at the
        // bottom of the screen (so no repaint on the first wrap): 240 cells.
        let row1 = "r".repeat(120);
        let row2 = "s".repeat(120);
        let stream = format!("\n{row1}{row2}\r\n\x1b[29;120Hs-and-more|0\r\n");
        let expect = format!("\n{row1}{row2}-and-more|0\r\n");
        // Width seeded from the pty size.
        let mut rows = RowNormalizer::with_width(120);
        let mut out = rows.push(&stream);
        out.push_str(&rows.finish());
        assert_eq!(out, expect);
        // Width unknown: the cumulative column is a multiple of the CUP column.
        let mut rows = RowNormalizer::default();
        let mut out = rows.push(&stream);
        out.push_str(&rows.finish());
        assert_eq!(out, expect);
        // A resize to another width is followed.
        let mut rows = RowNormalizer::with_width(120);
        rows.set_width(40);
        let line = "x".repeat(39) + "y";
        let mut out = rows.push(&format!("{line}\r\n\x1b[9;40Hyzzz\r\nnext"));
        out.push_str(&rows.finish());
        assert_eq!(out, format!("{line}zzz\r\nnext"));
    }

    #[test]
    fn wide_characters_take_two_cells_when_the_column_is_followed() {
        assert_eq!(char_cells('a'), 1);
        assert_eq!(char_cells('\u{e9}'), 1);
        assert_eq!(char_cells('\u{65e5}'), 2);
        assert_eq!(char_cells('\u{ff21}'), 2);
        assert_eq!(char_cells('\u{1F600}'), 2);
        assert_eq!(char_cells('\u{301}'), 0);
        assert_eq!(char_cells('\u{200d}'), 0);
        // 48 CJK characters (96 cells) and 24 ASCII fill a 120-cell row; ConPTY
        // then repaints at column 120. A one-cell-per-character count (72) never
        // reaches it and splits the row.
        let row = "\u{65e5}".repeat(48) + &"a".repeat(23) + "b";
        let mut rows = RowNormalizer::with_width(120);
        let mut out = rows.push(&format!("{row}\r\n\x1b[29;120Hb-more\r\n"));
        out.push_str(&rows.finish());
        assert_eq!(out, format!("{}-more\r\n", row));
        // The repainted last character may be non-ASCII too.
        let row = "a".repeat(118) + "\u{65e5}";
        let mut rows = RowNormalizer::with_width(120);
        let mut out = rows.push(&format!("{row}\r\n\x1b[29;120H\u{65e5}-more\r\n"));
        out.push_str(&rows.finish());
        assert_eq!(out, format!("{row}-more\r\n"));
    }

    #[test]
    fn a_held_break_is_released_by_a_flush_and_waits_longer_than_a_slow_write() {
        // The break is held until the next read says what it is; the reader
        // releases it after HELD_BREAK_FLUSH, which is well above a loaded
        // machine's gap between ConPTY's CR LF and the repaint that follows.
        assert!(HELD_BREAK_FLUSH >= std::time::Duration::from_millis(200));
        let line = "w".repeat(30);
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push(&format!("{line}\r\n")), line);
        // The repaint arrives in the next read, later than 40 ms would have waited.
        assert_eq!(rows.push("\x1b[9;30Hwmore"), "more");
        let mut rows = RowNormalizer::default();
        assert_eq!(rows.push(&format!("{line}\r\n")), line);
        assert_eq!(rows.flush_held(), "\r\n");
    }

    #[test]
    fn a_panic_in_the_reader_still_reports_the_exit() {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        spawn_reader_with_exit(
            ScriptedRead { steps: vec![Ok(b"boom".to_vec()), Ok(b"after".to_vec())], at: 0, done: None },
            Arc::new(AtomicU16::new(PTY_COLS)),
            move |event: ReaderEvent| {
                if event == ReaderEvent::Text("boom".into()) {
                    panic!("consumer bug");
                }
                tx.lock().unwrap().send(event).unwrap();
            },
            move || {
                let _ = done_tx.send(());
            },
        );
        done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("on_exit was not called after a panic");
        drop(rx);
    }
}
