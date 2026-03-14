---
title: Terminal and PTY
description: PTY lifecycle, completion detection, terminal events, and multi-session management in CommandUI.
sidebar:
  order: 15
---

CommandUI wraps a real pseudo-terminal (PTY). This is not a simulated shell — it's your actual system shell with full process management.

## PTY lifecycle

1. **Session creation:** `session_create` spawns a new PTY process with the detected shell (PowerShell on Windows, `$SHELL` on Unix)
2. **Ready signal:** the backend detects the shell prompt and emits `session:ready` with the initial cwd
3. **Command execution:** `terminal_execute` writes the command to the PTY and tracks it with an execution ID
4. **Output streaming:** PTY output is emitted line-by-line as `terminal:line` events
5. **Completion detection:** prompt-marker injection detects when the command finishes, extracting exit code
6. **Session close:** `session_close` terminates the PTY process

## Completion detection

CommandUI injects invisible prompt markers around commands to detect completion:

1. Before executing, a unique marker is written to the PTY
2. The command runs
3. After execution, the shell prompt reappears with the marker
4. The backend matches the marker, extracts the exit code, and emits `terminal:execution_finished`

**Limitation:** this breaks if the user has a custom shell prompt that strips or modifies the injected markers.

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
- **interrupting:** Ctrl+C sent, waiting for the process to exit
- **desynced:** terminal state lost (e.g., after a long-running process that corrupts markers). A "Resync" button appears for manual recovery.

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
