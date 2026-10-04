use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub const PROMPT_MARKER: &str = "__COMMANDUI_PROMPT__";

/// The key that submits a line to a PTY. A Windows ConPTY submits on CR
/// (Enter); LF is Ctrl+J there and never runs the line. Unix ptys translate
/// CR to LF (ICRNL) and readline/zle accept it. Every line runtime-core builds
/// itself ends in this; `write_raw` (the user's own keystrokes) never adds it.
pub(crate) const ENTER: &str = "\r";

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
    // `cmd /v:on /c` child instead (see `cmd_marker_value`).
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    cmd
}

/// Exit-code field of a marker that only says "a prompt was drawn" (cmd's
/// PROMPT cannot expand ERRORLEVEL). It finishes nothing; it returns a session
/// that was booting, resynced or running a command the user typed to Ready.
pub(crate) const PROMPT_ONLY_EXIT: i32 = i32::MIN;
/// How that marker spells its exit-code field. One character: a marker row
/// that wraps at the console width cannot be parsed.
pub(crate) const PROMPT_ONLY_FIELD: &str = "P";

/// Name of the cmd variable that holds the marker plumbing, so the line the
/// console echoes for an executed command ends in a short ` & %__cui%`.
const CMD_VAR: &str = "__cui";

/// Name of the cmd variable holding `(call )`, which sets ERRORLEVEL to 0.
/// cmd's built-ins (echo, cd, set) never reset ERRORLEVEL, so without it an
/// approved `echo` after a failure, or after Ctrl+C, would report that old code.
const CMD_RESET_VAR: &str = "__cuz";

/// What the console echoes in front of an executed cmd command: the
/// ERRORLEVEL reset. It is dropped from what is shown.
pub(crate) const CMD_RESET_ECHO: &str = "%__cuz% & ";

/// What the console echoes after an executed cmd command: the chained marker
/// plumbing. It is dropped from what is shown.
pub(crate) const CMD_PLUMBING_ECHO: &str = " & %__cui%";
/// The same, as a line of its own (the fallback for commands that cannot be
/// chained).
pub(crate) const CMD_MARKER_VAR_ECHO: &str = "%__cui%";

/// The text of `%__cui%` for cmd: capture the exit code, then print the marker
/// from a one-shot `cmd /v:on /c` child so delayed expansion never touches the
/// interactive session. `!CD!` is expanded by the child and its value is not
/// re-parsed, so a path containing `& ! ^ |` is safe. The carets are tripled
/// because the parent shell consumes one level. The value is set inside quotes
/// (where `&`, `^` and `|` are literal) and is expanded when a typed line
/// containing `%__cui%` is parsed.
fn cmd_marker_value(nonce: &str) -> String {
    format!(
        "call set __cui_ec=%^ERRORLEVEL% & \"%ComSpec%\" /v:on /c echo. ^& echo {PROMPT_MARKER}^^^|{nonce}^^^|!CD!^^^|!__cui_ec!"
    )
}

/// cmd's own prompt: a marker line with the cwd, then the usual `$P$G`.
fn cmd_prompt_command(nonce: &str) -> String {
    format!("prompt {PROMPT_MARKER}^|{nonce}^|$P^|{PROMPT_ONLY_FIELD}$_$P$G")
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
            "function prompt {{ $__cui_ok = $?; $__cui_code = $global:LASTEXITCODE; if ($__cui_ok) {{ $__cui_code = 0 }} elseif (-not ($__cui_code -is [int]) -or $__cui_code -eq 0) {{ $__cui_code = 1 }}; try {{ if ((Get-PSReadLineOption).EditMode -eq 'Vi') {{ Set-PSReadLineKeyHandler -ViMode Insert -Chord 'Ctrl+]' -Function RevertLine; Set-PSReadLineKeyHandler -ViMode Command -Chord 'Ctrl+]' -ScriptBlock {{ [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine(); [Microsoft.PowerShell.PSConsoleReadLine]::ViInsertMode() }} }} else {{ Set-PSReadLineKeyHandler -Chord 'Ctrl+]' -Function RevertLine }} }} catch {{ }}; $__cui_cwd = (Get-Location).Path.Replace('%','%25').Replace([string][char]13,'%0D').Replace([string][char]10,'%0A'); $__cui_line = ([string][char]10) + '{PROMPT_MARKER}|{nonce}|' + $__cui_cwd + '|' + $__cui_code; \"$__cui_line`n> \" }}{ENTER}"
        )),
        ShellFamily::Cmd => Some(format!(
            "set \"{CMD_VAR}={}\"{ENTER}set \"{CMD_RESET_VAR}=(call )\"{ENTER}{}{ENTER}",
            cmd_marker_value(nonce),
            cmd_prompt_command(nonce)
        )),
        ShellFamily::Bash => Some(format!(
            "bind -m emacs '\"\\C-]\": kill-whole-line'; bind -m vi-insert '\"\\C-]\": kill-whole-line'; bind -m vi-command '\"\\C-]\": \"A\\C-u\"'; __cui_nl=$'\\n'; __cui_cr=$'\\r'; PROMPT_COMMAND='__cui_ec=$?; __cui_cwd=${{PWD//\\%/%25}}; __cui_cwd=${{__cui_cwd//$__cui_nl/%0A}}; __cui_cwd=${{__cui_cwd//$__cui_cr/%0D}}; printf \"\\n{PROMPT_MARKER}|{nonce}|%s|%s\\n\" \"$__cui_cwd\" \"$__cui_ec\"'{ENTER}"
        )),
        ShellFamily::Zsh => Some(format!(
            "precmd() {{ local __cui_ec=$? __cui_cwd=\"${{PWD}}\"; __cui_cwd=${{__cui_cwd//\\%/%25}}; __cui_cwd=${{__cui_cwd//$'\\n'/%0A}}; __cui_cwd=${{__cui_cwd//$'\\r'/%0D}}; print -r -- \"\"; print -r -- \"{PROMPT_MARKER}|{nonce}|${{__cui_cwd}}|${{__cui_ec}}\" }}{ENTER}"
        )),
        ShellFamily::Unsupported => None,
    }
}

/// Bytes written for one executed command. cmd chains a marker after the
/// command on the same line because its prompt cannot expand ERRORLEVEL. The
/// command text is the command, not a format string.
pub(crate) fn command_line_for_shell(shell: &str, _nonce: &str, command: &str) -> String {
    let clear = clear_input_line(shell);
    if shell_family(shell) == ShellFamily::Cmd {
        // `%__cui%` (set at bootstrap) captures the exit code with `call`
        // after the command ran, then prints the marker. On the same line so
        // a program that flushes the console input buffer (pause, choice,
        // set /p) cannot eat it.
        if cmd_can_chain(command) {
            format!("{clear}%{CMD_RESET_VAR}% & {command} & %{CMD_VAR}%{ENTER}")
        } else {
            // A trailing rem, ::, ^, an open quote or paren, or an if/for
            // that would take the chained marker into its own body: the
            // marker goes on a line of its own instead.
            format!("{clear}%{CMD_RESET_VAR}% & {command}{ENTER}%{CMD_VAR}%{ENTER}")
        }
    } else {
        format!("{clear}{command}{ENTER}")
    }
}

/// Can ` & marker` be appended to this cmd line and still run once, always,
/// after the command? Not when the command could swallow or capture it.
fn cmd_can_chain(command: &str) -> bool {
    if command.matches('"').count() % 2 != 0 {
        return false;
    }
    let trimmed = command.trim_end();
    if trimmed.ends_with('^') || trimmed.contains("::") {
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
    !command
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| matches!(word, "rem" | "if" | "for"))
}

/// The key chord that clears the input line whatever the editor is doing. It
/// is bound at bootstrap, in every editing mode, to "throw away the line and
/// be ready to insert": PowerShell binds it to RevertLine in the emacs, windows
/// and both vi keymaps (re-bound at each prompt, so a later
/// `Set-PSReadLineOption -EditMode` does not lose it), bash binds it with
/// `bind` in the emacs, vi-insert and vi-command keymaps. A vi command mode
/// has no key that clears a line, so no fixed run of ordinary keys could work
/// there.
pub(crate) const CLEAR_CHORD: &str = "\x1d";

/// Bytes that discard whatever the user already typed at the prompt, written
/// in the same write as an approved command so it cannot be appended to a
/// half-typed line (`rm -rf ` + approved `ls` must not run `rm -rf ls`).
///
/// bash and PowerShell: the bound chord above (PowerShell then also gets
/// Ctrl+End, Ctrl+Home for a console without PSReadLine).
/// zsh (zle): Ctrl+E (end of line) then Ctrl+U (kill to start).
///
/// cmd: Ctrl+End then Ctrl+Home, as the VT input sequences `CSI 1;5 F` and
/// `CSI 1;5 H`; its console line editor deletes to the end / start of the
/// line, so the whole line goes whatever the cursor position. Escape is NOT
/// usable: a ConPTY reads ESC followed by more bytes in the same write as
/// Alt+<key>, so the first letter of the command was swallowed (`xyz` + ESC +
/// `echo ok` ran `xyzecho ok`). The CSI forms are unambiguous in a single write.
pub(crate) fn clear_input_line(shell: &str) -> String {
    match shell_family(shell) {
        ShellFamily::Bash => CLEAR_CHORD.to_string(),
        ShellFamily::Zsh => "\x05\x15".to_string(),
        ShellFamily::PowerShell => format!("{CLEAR_CHORD}\x1b[1;5F\x1b[1;5H"),
        ShellFamily::Cmd => "\x1b[1;5F\x1b[1;5H".to_string(),
        ShellFamily::Unsupported => String::new(),
    }
}

/// What is written to make a session report a prompt again. cmd's PROMPT and
/// the other shells' prompt hooks print the marker on their own.
pub(crate) fn resync_input(_shell: &str, _nonce: &str) -> String {
    ENTER.to_string()
}

/// A Windows ConPTY repaints by position: instead of `\r\n` it moves to the
/// start of the next row with a cursor-position escape (`CSI row;col H`), so a
/// finished line (the output of a command, the prompt marker) can arrive with
/// no line break at all. The runtime reads output by line, so this turns such
/// a cursor-position escape into a line break when the row it leaves has text
/// on it and the cursor moves to a different row, and drops it otherwise.
/// A cursor-position escape that stays on the row the cursor is already on
/// (an in-line redraw) passes through, as does every other escape. The row is
/// followed from the escapes seen and from line feeds (a ConPTY soft wrap is
/// not a new row: see "Soft wraps" below; an unknown row counts as different).
/// While the alternate screen is active (`CSI ? 1049/1047/47 h`, alone or
/// combined with other modes) nothing is rewritten: vim, less and htop need
/// their cursor positioning. Prompts and markers never appear there.
/// An escape sequence cut in half by a read is held for the next one.
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
/// too. The column is followed from the text written, so this holds at any
/// console width and needs no width to be told. A `CR` or `LF` on a row this
/// wide is held until the next read shows which it is (`flush_held` releases
/// it when nothing follows).
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
    /// The console width, once a soft wrap has shown it. Text written past it
    /// continues on the next row, as the terminal does.
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
/// column `cand`? See `RowNormalizer`.
fn wrap_lookahead(input: &str, i: usize, cand: usize) -> Wrap {
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
                            Some(c) if c == cand && c > 1 => {
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

    fn push_inner(&mut self, text: &str, force: bool) -> String {
        let mut input = std::mem::take(&mut self.carry);
        self.holding = false;
        input.push_str(text);
        let bytes = input.as_bytes();
        let mut out = String::with_capacity(input.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != 0x1b {
                // A prompt marker always starts its own row, in or out of the
                // alternate screen, and always ends the alternate screen: a
                // program that was killed never sent its `?1049l`.
                let rest = &input[i..];
                if rest.starts_with(PROMPT_MARKER) {
                    self.alt_screen = false;
                    if self.row_has_text {
                        out.push('\n');
                        self.row_has_text = false;
                        self.col = 0;
                    }
                } else if self.alt_screen
                    && rest.len() < PROMPT_MARKER.len()
                    && PROMPT_MARKER.starts_with(rest)
                {
                    // Possibly the start of a marker cut by the read.
                    self.carry = rest.to_string();
                    break;
                }
                let ch = rest.chars().next().unwrap();
                if (ch == '\r' || ch == '\n') && !self.alt_screen {
                    let cand = if ch == '\n' && self.col == 0 { self.col_before_cr } else { self.col };
                    if cand >= MIN_WRAP_COL {
                        match wrap_lookahead(&input, i, cand) {
                            Wrap::Yes { pass, end, row, col } => {
                                out.push_str(&input[pass.0..pass.1]);
                                i = end;
                                self.row = row;
                                self.row_has_text = true;
                                self.col = col - 1;
                                self.width = Some(col);
                                self.drop_dup = self.last_char.filter(|c| c.is_ascii());
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
                    if self.drop_dup.take() == Some(ch) {
                        self.col += 1;
                        i += ch.len_utf8();
                        continue;
                    }
                    if self.width.is_some_and(|w| self.col >= w) {
                        self.col = 0;
                    }
                    self.col += 1;
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
                    // a marker or cursor jump later in the read still counts.
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
        // The read blocks, and a CR/LF held back by the normalizer has to be
        // released when nothing follows it, so the read gets a thread of its
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
        // A panic anywhere in here (a bug in the normalizer or a consumer)
        // must not leave the session "running" forever: on_exit always runs.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Bytes of a multibyte character that a read cut in half.
            let mut pending: Vec<u8> = Vec::new();
            let mut rows = RowNormalizer::default();
            loop {
                match rx.recv_timeout(HELD_BREAK_FLUSH) {
                    Ok(bytes) => {
                        let mut text = decode_utf8_stream(&mut pending, &bytes);
                        if cfg!(windows) {
                            let raw = text.clone();
                            // The normalizer is only a cosmetic rewrite: if it
                            // ever panics, keep reading and pass the text on.
                            text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rows.push(&raw)))
                                .unwrap_or_else(|_| {
                                    rows = RowNormalizer::default();
                                    raw
                                });
                        }
                        if !text.is_empty() {
                            on_chunk(text);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if cfg!(windows) {
                            let text = rows.flush_held();
                            if !text.is_empty() {
                                on_chunk(text);
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let mut rest = String::new();
            if !pending.is_empty() {
                rest.push_str(&String::from_utf8_lossy(&pending));
                if cfg!(windows) {
                    rest = rows.push(&rest);
                }
            }
            rest.push_str(&rows.finish());
            if !rest.is_empty() {
                on_chunk(rest);
            }
        }));
        on_exit();
    });
}

/// How long a CR/LF held back by the normalizer waits for the next read.
const HELD_BREAK_FLUSH: std::time::Duration = std::time::Duration::from_millis(40);

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
        assert!(cmd.contains("%^ERRORLEVEL%"));
        assert!(cmd.contains("!CD!"));
        assert!(cmd.contains(NONCE));
        assert!(cmd.contains("^|"));
        // cmd's own prompt prints a prompt-only marker, so a command the user
        // typed by hand is noticed to have ended.
        assert!(cmd.contains(&format!("prompt {PROMPT_MARKER}^|{NONCE}^|$P^|P$_$P$G")), "{cmd}");

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
    fn bootstrap_binds_the_clear_chord_in_every_editing_mode() {
        let ps = bootstrap_prompt("pwsh", NONCE).unwrap();
        assert!(ps.contains("-ViMode Insert -Chord 'Ctrl+]' -Function RevertLine"), "{ps}");
        assert!(ps.contains("-ViMode Command -Chord 'Ctrl+]'"), "{ps}");
        assert!(ps.contains("ViInsertMode()"), "{ps}");
        assert!(ps.contains("-Chord 'Ctrl+]' -Function RevertLine"), "{ps}");
        // Re-bound at every prompt, so a later -EditMode switch keeps it.
        assert!(ps.find("Set-PSReadLineKeyHandler").unwrap() < ps.find(PROMPT_MARKER).unwrap());
        let bash = bootstrap_prompt("/bin/bash", NONCE).unwrap();
        for map in ["emacs", "vi-insert", "vi-command"] {
            assert!(bash.contains(&format!("bind -m {map} ")), "{map}: {bash}");
        }
        assert!(bash.contains("kill-whole-line"));
        assert_eq!(CLEAR_CHORD, "\x1d");
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
    fn cmd_command_line_chains_the_marker_on_the_same_line() {
        let nasty = "echo %CD% & del /q *";
        let line = command_line_for_shell("cmd.exe", NONCE, nasty);
        // One line, one Enter: a program that flushes the console input
        // buffer (pause, choice, set /p) has no typed-ahead marker to eat.
        assert_eq!(line, format!("\x1b[1;5F\x1b[1;5H%__cuz% & {nasty} & %__cui%\r"));
        assert_eq!(line.matches('\r').count(), 1);
        assert!(!line.contains('\n'), "a Windows ConPTY submits on CR; LF is Ctrl+J");
        // The marker plumbing is in the bootstrap's variable, with the nonce.
        let boot = bootstrap_prompt("cmd.exe", NONCE).unwrap();
        assert!(boot.contains(&format!("{PROMPT_MARKER}^^^|{NONCE}^^^|!CD!^^^|!__cui_ec!")), "{boot}");
        assert!(boot.starts_with("set \"__cui=call set __cui_ec=%^ERRORLEVEL% & "), "{boot}");
        // ...and so is the ERRORLEVEL reset (`(call )` sets it to 0).
        assert!(boot.contains("\rset \"__cuz=(call )\"\r"), "{boot}");
        assert_eq!(command_line_for_shell("bash", NONCE, "ls"), "\x1dls\r");
        assert_eq!(resync_input("bash", NONCE), "\r");
        assert_eq!(resync_input("cmd.exe", NONCE), "\r");
    }

    #[test]
    fn cmd_commands_that_could_swallow_the_chained_marker_use_a_line_of_their_own() {
        for command in [
            "echo hi & rem",
            "echo hi :: comment",
            "echo \"unbalanced",
            "echo hi ^",
            "if exist x echo y",
            "for /l %i in (1,1,3) do @echo %i",
            "(echo a",
            "echo a)",
        ] {
            let line = command_line_for_shell("cmd.exe", NONCE, command);
            assert_eq!(line, format!("\x1b[1;5F\x1b[1;5H%__cuz% & {command}\r%__cui%\r"), "{command}");
        }
        for command in ["echo hi", "dir /b", "cd /d \"C:\\a b\"", "echo (a) & echo b", "git status"] {
            let line = command_line_for_shell("cmd.exe", NONCE, command);
            assert!(line.ends_with(" & %__cui%\r"), "{command}: {line:?}");
        }
    }

    #[test]
    fn command_line_clears_pending_input_per_shell_family() {
        assert_eq!(command_line_for_shell("zsh", NONCE, "ls"), "\x05\x15ls\r");
        assert_eq!(command_line_for_shell("pwsh.exe", NONCE, "ls"), "\x1d\x1b[1;5F\x1b[1;5Hls\r");
        assert_eq!(command_line_for_shell("powershell.exe", NONCE, "ls"), "\x1d\x1b[1;5F\x1b[1;5Hls\r");
        assert_eq!(command_line_for_shell("/bin/bash", NONCE, "ls"), "\x1dls\r");
        assert_eq!(command_line_for_shell("cmd.exe", NONCE, "dir"), "\x1b[1;5F\x1b[1;5H%__cuz% & dir & %__cui%\r");
    }

    #[test]
    fn cmd_commands_with_bangs_are_written_through_unchanged() {
        for command in ["echo hello!", "git commit -m \"done!\"", "cd hello!world"] {
            let line = command_line_for_shell("cmd.exe", NONCE, command);
            assert_eq!(line, format!("\x1b[1;5F\x1b[1;5H%__cuz% & {command} & %__cui%\r"), "{line}");
            // Only the marker child expands `!`; the session itself never does.
            assert!(
                bootstrap_prompt("cmd.exe", NONCE).unwrap().contains("\"%ComSpec%\" /v:on /c"),
                "{line}"
            );
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
        assert!(bootstrap_prompt("cmd.exe", NONCE).unwrap().contains("$_$P$G"));
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
    fn every_line_runtime_core_builds_is_submitted_with_cr_not_lf() {
        // A Windows ConPTY runs a line on CR (Enter); LF is Ctrl+J there and
        // the bootstrap was typed and never run.
        for shell in ["powershell.exe", "pwsh", "cmd.exe", "/bin/bash", "/bin/zsh"] {
            let boot = bootstrap_prompt(shell, NONCE).unwrap();
            assert!(boot.ends_with('\r'), "{shell}: {boot:?}");
            assert!(!boot.contains('\n'), "{shell}: {boot:?}");
            let line = command_line_for_shell(shell, NONCE, "ls");
            assert!(line.ends_with('\r'), "{shell}: {line:?}");
            assert!(!line.contains('\n'), "{shell}: {line:?}");
            let resync = resync_input(shell, NONCE);
            assert!(resync.ends_with('\r'), "{shell}: {resync:?}");
            assert!(!resync.contains('\n'), "{shell}: {resync:?}");
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
    fn clear_line_is_a_csi_pair_never_a_lone_escape_on_windows_shells() {
        // ESC followed by more bytes in one write is Alt+<key> to a ConPTY:
        // `xyz` + ESC + `echo ok` ran `xyzecho ok`. Ctrl+End / Ctrl+Home as
        // CSI sequences are unambiguous.
        assert_eq!(clear_input_line("cmd.exe"), "\x1b[1;5F\x1b[1;5H");
        // PowerShell and bash clear with a chord bound at bootstrap in every
        // editing mode (a vi command mode has no ordinary key that clears).
        for shell in ["powershell.exe", "pwsh.exe"] {
            assert_eq!(clear_input_line(shell), "\x1d\x1b[1;5F\x1b[1;5H", "{shell}");
        }
        assert_eq!(clear_input_line("/bin/bash"), "\x1d");
        assert_eq!(clear_input_line("zsh"), "\x05\x15");
        assert_eq!(clear_input_line("fish"), "");
    }

    #[test]
    fn row_normalizer_turns_cursor_positioning_into_line_breaks() {
        let mut rows = RowNormalizer::default();
        // ConPTY: output, then the marker on the next row with no CR LF.
        assert_eq!(
            rows.push("probe-ok\x1b[?25l\x1b[15;1H__COMMANDUI_PROMPT__|n|C:\\w|0\x1b[16;1HC:\\w>"),
            "probe-ok\x1b[?25l\n__COMMANDUI_PROMPT__|n|C:\\w|0\nC:\\w>"
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
    fn a_marker_ends_the_alternate_screen_and_always_starts_its_own_row() {
        // The program was killed: it never sent ?1049l. Its screen text is
        // followed, with no row change, by the next prompt marker.
        let mut rows = RowNormalizer::default();
        let out = rows.push("\x1b[?1049hscreen text__COMMANDUI_PROMPT__|n|C:\\w|0\r\nnext\x1b[9;1Hmore");
        assert_eq!(
            out,
            "\x1b[?1049hscreen text\n__COMMANDUI_PROMPT__|n|C:\\w|0\r\nnext\nmore"
        );
        // And the normalizer is back to rewriting cursor jumps.
        assert!(!rows.alt_screen);
        // A marker split by a read inside the alternate screen is held, not
        // glued to the screen text.
        let mut rows = RowNormalizer::default();
        let mut out = rows.push("\x1b[?1049hdraw__COMMANDUI_PRO");
        out.push_str(&rows.push("MPT__|n|C:\\w|0\r\n"));
        // The row is wide enough to be a soft wrap, so its line break waits for
        // the next read (or the flush) to say which it is.
        out.push_str(&rows.flush_held());
        assert_eq!(out,"\x1b[?1049hdraw\n__COMMANDUI_PROMPT__|n|C:\\w|0\r\n");
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
    fn a_panic_in_the_reader_still_reports_the_exit() {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        spawn_reader_with_exit(
            ScriptedRead { steps: vec![Ok(b"boom".to_vec()), Ok(b"after".to_vec())], at: 0, done: None },
            move |chunk: String| {
                if chunk == "boom" {
                    panic!("consumer bug");
                }
                tx.lock().unwrap().send(chunk).unwrap();
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
}
