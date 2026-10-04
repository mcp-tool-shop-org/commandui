# Terminal and PTY

CommandUI wraps a real pseudo-terminal (PTY). This is not a simulated shell — it's your actual system shell with full process management.

## PTY lifecycle

1. **Session creation:** `session_create` spawns a new PTY process with the detected shell (on Windows, PowerShell 7 if it is installed or on PATH, otherwise Windows PowerShell; on Unix, `$SHELL`). cmd, bash (including Git Bash on Windows) and zsh are also supported
2. **Ready signal:** the backend installs a prompt hook in the shell and emits `session:ready` with the initial cwd when the first prompt reports in
3. **Command execution:** `terminal_execute` clears anything half-typed at the prompt, writes the command to the PTY, and tracks it with an execution ID
4. **Output streaming:** PTY output is emitted as `terminal:line` events, including partial lines (a prompt waiting for input appears at once)
5. **Completion detection:** the shell's prompt hook reports the exit code and cwd in an invisible escape sequence
6. **Session close:** `session_close` terminates the PTY process

## Completion detection

At startup CommandUI installs a prompt hook in the shell: the `prompt` function in PowerShell, `PROMPT_COMMAND` in bash, `precmd` in zsh, and the `PROMPT` setting in cmd. Every time the shell shows its prompt, the hook writes an OSC escape sequence (`ESC ] 7733 ; … BEL`) carrying a per-session nonce, the last exit code and the percent-encoded cwd. An OSC sequence takes no columns, so the terminal never wraps or repaints it; the backend reads it from the raw output stream, never displays it, checks the nonce, and emits `terminal:execution_finished` and `session:cwd_changed`.

cmd cannot expand `%ERRORLEVEL%` inside its prompt, so for cmd an approved command line also carries a short hidden tail that records the exit code, and starts with `(call )` so a built-in like `echo` does not report the previous command's code.

Because the prompt hook reports after every command, commands you type yourself are tracked too: pressing Enter at a ready prompt puts the session in `userRunning` until the next prompt.

**Limitation:** a prompt framework that replaces the `prompt` function, `PROMPT_COMMAND` or `precmd` after CommandUI installs its hook stops completion detection. The session then stays `running`; Interrupt or Resync recovers it.

## Terminal events

| Event | Payload | When |
|-------|---------|------|
| `terminal:line` | `sessionId`, `executionId?`, `text` | Each line of PTY output |
| `terminal:execution_started` | `execution: { id, sessionId, command }` | Command begins |
| `terminal:execution_finished` | `executionId`, `sessionId`, `status`, `exitCode` | Command completes |
| `session:cwd_changed` | `sessionId`, `cwd` | Working directory changes |
| `session:ready` | `sessionId`, `cwd` | Shell initialized |
| `session:exec_state_changed` | `sessionId`, `execState`, `changedAt` | State machine transition |

## Execution state machine

Each session tracks its execution state:

```
booting → ready ⇄ running
                ↓
          interrupting → ready
                ↓
           desynced → (resync) → ready
```

- **booting:** PTY is starting, shell not yet responsive
- **ready:** idle, accepting commands
- **running:** a command is executing
- **userRunning:** a command you typed yourself is running (`ssh`, `vim`, `make`): the session entered it when your keystrokes included Enter, and leaves it when the shell's prompt returns. Approved commands are refused while it lasts, since they would be typed into that program. The desktop shows a "running (typed)" badge; Interrupt works
- **interrupting:** Ctrl+C sent, waiting for the process to exit
- **desynced:** terminal state lost (e.g., after a long-running process that corrupts markers). A "Resync" button appears for manual recovery.

A session that stays in `booting` for 20 seconds shows a banner with **Resync** and **Close Session**, so a shell that never starts is not a dead pane.

Before an approved command, the runtime clears anything half-typed at the prompt so it cannot be joined to the command. In bash and PowerShell sessions it binds `Ctrl+]` to "discard the current line" (in every editing mode, vi included) and sends that chord, but only when `Ctrl+]` is unbound or still has the shell's default (character search): a binding of your own is left alone, and the runtime then falls back to `Ctrl+End Ctrl+Home` (PowerShell) or `Ctrl+E Ctrl+U` (bash), which does not clear a vi normal-mode line. zsh gets `Ctrl+E Ctrl+U`, cmd gets `Ctrl+End Ctrl+Home`.

When the shell process itself exits (`exit`, a crash, a killed process), the session's status becomes `exited` and its execution state `desynced`, but this is not recoverable: `execute` and `resync` refuse the session. The desktop shows a banner with a **New Session** button in place of Resync. Exit is detected both from the end of the PTY output and by polling the child process, since a Windows ConPTY may not end the output stream.

## xterm.js rendering

The frontend uses xterm.js to render terminal output:

- Real terminal emulation with ANSI color support
- Fit addon for responsive resizing
- Session-switch replay: when switching tabs, buffered output is replayed to the terminal
- Terminal resize events are forwarded to the PTY backend for proper column/row handling

## Interrupt handling

When you press the Interrupt button (or the running command's cancel action):

1. `terminal_interrupt` sends SIGINT to the PTY process group
2. Execution state transitions to `interrupting`
3. If the process exits, `terminal:execution_finished` fires with status `interrupted`
4. State returns to `ready`

## Multi-session

Each tab is an independent PTY session with its own:
- Shell process
- Working directory
- Execution state
- Terminal output buffer

Session switching replays buffered output to the xterm instance. Sessions are created with `Ctrl+T` and closed with `Ctrl+W` (Tauri only).
