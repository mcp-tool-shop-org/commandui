# CommandUI Console

A terminal shell with sidecar AI assistance. You play freely in a real PTY shell, and when you want help — launching, configuring, understanding — the AI is right there, without ever getting in the way.

## What it does

- **Line-oriented shell play** — real PTY sessions in your shell of choice (bash, zsh, PowerShell, pwsh, cmd). Prompts and typed input appear before the line ends, and edits on the current line (backspace, cursor moves, erase-to-end, and the Windows console's same-row redraws) are followed
- **Raw Play mode** — fullscreen mode for terminal games and editors (vim, htop, lazygit). Console leaves its own screen and forwards keys, resizes, paste, mouse and focus events to the app. On Windows, apps that draw on the alternate screen (most full-screen apps) get their output byte for byte; an app that draws on the main screen has its cursor moves between rows turned into line breaks
- **Knows what is in the foreground** — a command you typed yourself (`ssh`, `vim`, `make`) marks the session FOREGROUND until the shell's prompt comes back. While it runs, Review refuses to approve a command (it would be typed into that program), and closing or quitting asks first
- **Ask / Review / Approve** — describe what you want in natural language, review the generated command and its risk assessment, then approve or cancel
- **Multi-session** — run multiple shell sessions, switch between them with a run selector, see state badges and unread markers
- **Session-bound proposals** — Ask records which session you're in, and approval runs the command there. Session switching is refused while Ask or Review is open
- **Every command approved by hand** — nothing runs until you press Enter or `y` in Review. A proposal the planner flags as needing confirmation also needs `c` first
- **Nothing stored** — sessions and transcripts live in memory for the life of the process. Console keeps no history, workflows, memory or settings, and opens no database

## What it is not

- **Not a terminal emulator** — Console wraps your shell via PTY. In Raw Play it hands the screen to the app and re-encodes host input for it, rather than emulating a terminal
- **Not a chatbot** — the AI generates executable shell commands with risk assessment, not conversation
- **Not a shell replacement** — it wraps your existing shell (bash, zsh, PowerShell, etc.)

## Controls

### Shell mode

| Key | Action |
|-----|--------|
| `^T` | Ask the AI (enter Ask mode) |
| `^G` | Enter Raw Play (fullscreen passthrough) |
| `^S` | Open run selector |
| `F1` / `^/` | Help overlay (`^H` is Backspace to a shell, so it goes to the shell) |
| `^N` | New session |
| `^W` | Close session (asks `y/n` if a command is running, approved or typed by you) |
| `^]` / `^[` | Next / previous session (on Unix, a bare `Esc` is `^[` when more than one session is open) |
| `^C` | Interrupt running command |
| `^R` | Resync session |
| `^Q` | Quit (asks `y/n` if any session has a running command, approved or typed by you) |
| `Shift+PgUp/PgDn` | Scroll terminal |

At a `y/n` prompt only a plain `y` goes ahead; any other key cancels and is not sent to the shell.

While a session is still starting (BOOT), keystrokes are not sent to it; `^R` resends Enter to a shell that is slow to show its first prompt, and `^W` / `^N` still close or create sessions.

### Ask mode

| Key | Action |
|-----|--------|
| `Enter` | Submit intent |
| `Esc` / `^T` | Back to Shell; while a proposal is generating, cancels it |
| `^U` | Clear input |

### Review mode

| Key | Action |
|-----|--------|
| `Enter` / `y` | Approve and run on the proposal's session |
| `c` | Confirm a proposal that requires it |
| `↑↓` / `jk` / `PgUp/PgDn` | Scroll the command |
| `Esc` / `n` | Cancel, back to Ask |
| `^Q` | Quit |

Approval is refused while the command is clipped, while confirmation is pending, for about 0.6 seconds after a proposal appears (so a key pressed while the planner was working cannot approve a command you have not seen), and when the session's directory has changed since you asked. `y` and `n` only count without Ctrl or Alt. Review shows the directory the command will run in on a "Runs in:" line, and flags commands that contain non-ASCII or invisible formatting characters.

### Run selector

| Key | Action |
|-----|--------|
| `↑↓` / `jk` | Navigate |
| `Enter` | Select session |
| `1-9` | Jump to a row in view |
| `^N` / `^W` | New session / close the highlighted session |
| `Esc` / `^S` | Close |

### Raw Play mode

| Key | Action |
|-----|--------|
| `^\` (Ctrl+Backslash) | Exit Raw Play, return to Console |
| `^Q` | Exit Raw Play and ask `y/n` before quitting (always asks here, since full-screen apps use `^Q` themselves) |
| Everything else | Forwarded to the game/app |

## Modes

**Shell** — Default mode. Keystrokes go to your shell. Console chrome (status bar, footer hints) is visible.

**Ask** — Intent composer. Type what you want in natural language, press Enter. Console sends your intent to the local AI planner, which generates a command proposal.

**Review** — Shows the generated command, risk level, confidence score, safety flags, and explanation. Approve to execute, cancel to go back and refine. If the local model can't be reached, a release build returns no command. A debug build may show a stand-in, and the Review title says the model call failed.

**Runs** — Run selector overlay. See all sessions with state badges (IDLE, RUN, FOREGROUND, BOOT, STOP, DONE, ERR!), unread markers, and CWD. Switch, create, or close sessions.

**Raw Play** — Fullscreen mode. Console leaves its alternate screen and the app draws to the host terminal directly. Keys, resizes, bracketed paste and focus events are forwarded in the encodings the app enabled. Mouse events reach apps that enable SGR mouse reporting (`?1006`), with drag and motion only when the app asks for them (`?1002` / `?1003`). Replies the host terminal sends to the app's own queries (cursor position, device attributes) are not forwarded yet. `^\` returns to Console; `^Q` returns and asks before quitting. If writing to the app fails, the session is marked as an error and Console comes back. The transcript keeps the text with control sequences stripped. Movement within a line is followed, but movement between lines is not, so full-screen output reads out of order.

## Architecture

Console is one of two shells in the CommandUI product family. Both shells are adapters over the same shared crates. Console uses runtime-core and runtime-planner; only Desktop uses runtime-persistence.

```
┌─────────────┐  ┌─────────────┐
│   Desktop   │  │   Console   │   ← Shells (adapters)
│  (Tauri)    │  │  (Ratatui)  │
└──────┬──────┘  └──────┬──────┘
       │                │
       └───────┬────────┘
               │
  ┌────────────┴────────────┐
  │     runtime-core        │  ← PTY, sessions, events, services
  ├─────────────────────────┤
  │  runtime-persistence    │  ← SQLite CRUD
  ├─────────────────────────┤
  │   runtime-planner       │  ← Ollama client, proposals, validation
  └─────────────────────────┘
```

The integration seam is `RuntimeEventSink` — Console implements it via a tokio channel; Desktop via Tauri events. Parity tests prevent silent drift between the two shells.

## Limitations

- **Shell mode is a line view, not a terminal** — it follows line edits on the current line, but not cursor addressing across lines or full-screen redraws. Use Raw Play for anything that draws the whole screen
- **Transcript degraded in Raw Play** — control sequences are stripped and movement between lines is not followed, so full-screen app output is kept as text, not as the screen you saw
- **No UI during Raw Play** — no split-screen, no real-time AI overlay. Exit Raw Play first to use Console features
- **Session switching blocked during Raw Play** — exit with `^\` first
- **Mouse in Raw Play needs SGR reporting** — apps that only use the older X10 or UTF-8 mouse encodings get no mouse events
- **No persistence, workflow UI, memory UI, or history reopen yet** — these exist in the Desktop shell but are deferred for Console
- **Console cannot detect a full-screen app quitting** — when the app exits but the shell lives on, you may need to press `^\` manually. A shell that exits is detected and shown
- **`Ctrl+]` may be rebound inside sessions** — to clear a half-typed line before an approved command in every editing mode (including vi), CommandUI binds `Ctrl+]` in PowerShell and bash sessions to "discard the current line", unless you have bound it to something yourself. Console itself uses `^]` for next session in Shell mode, so you only meet the shell binding in Raw Play
- **Your shell prompt may be replaced** — CommandUI installs its own prompt hook to detect when commands finish. In PowerShell the prompt becomes `> ` and in cmd `$P$G`, so a custom prompt theme is not shown; in bash your `PS1` stays, but a prompt tool that runs from `PROMPT_COMMAND` (starship, for example) is replaced
- **Runtime messages go to a log** — while Console owns the terminal, diagnostic output from the runtime is written to `commandui-console-stderr.log` in the system temp directory instead of over the screen

## Build and test

```bash
cargo test -p commandui-console    # Run all Console tests
cargo run -p commandui-console     # Launch Console
cargo check -p commandui-console   # Type-check without building
```

## Doctrine

Console's design is governed by these specs:

- [Play Law](../../docs/specs/play-law.md) — terminal-native play experiences, sidecar assistance model
- [Parity Law](../../docs/specs/parity-law.md) — shared truth contract between Desktop and Console
- [Raw Play Mode](../../docs/specs/raw-play-mode.md) — host terminal passthrough, transcript law
- [TUI Spine](../../docs/specs/tui-spine.md) — extraction architecture, adapter model
