# CommandUI Console

A terminal shell with sidecar AI assistance. You play freely in a real PTY shell, and when you want help — launching, configuring, understanding — the AI is right there, without ever getting in the way.

## What it does

- **Line-oriented shell play** — real PTY sessions, your shell of choice, full interactive behavior
- **Raw Play mode** — fullscreen passthrough for ncurses apps, terminal games, editors (vim, htop, lazygit). Console steps aside; the game gets the real host terminal
- **Ask / Review / Approve** — describe what you want in natural language, review the generated command and its risk assessment, then approve or cancel
- **Multi-session** — run multiple shell sessions, switch between them with a run selector, see state badges and unread markers
- **Session-bound proposals** — Ask captures which session you're in. Approval executes on that session, even if you switched away
- **Risk-tiered confirmation** — low-risk commands auto-execute (configurable), medium prompts, high always confirms
- **Local persistence** — history, plans, workflows, memory, and settings stored in SQLite. No cloud, no sync

## What it is not

- **Not a terminal emulator** — Console wraps your shell via PTY. In Raw Play, it surrenders to the host terminal entirely (passthrough, not emulation)
- **Not a chatbot** — the AI generates executable shell commands with risk assessment, not conversation
- **Not a shell replacement** — it wraps your existing shell (bash, zsh, PowerShell, etc.)

## Controls

### Shell mode

| Key | Action |
|-----|--------|
| `^T` | Ask the AI (enter Ask mode) |
| `^G` | Enter Raw Play (fullscreen passthrough) |
| `^S` | Open run selector |
| `^H` | Help overlay |
| `^N` | New session |
| `^W` | Close session |
| `^]/^[` | Next / previous session |
| `^C` | Interrupt running command |
| `^R` | Resync session |
| `^Q` | Quit |
| `Shift+PgUp/PgDn` | Scroll terminal |

### Ask mode

| Key | Action |
|-----|--------|
| `Enter` | Submit intent |
| `Esc` | Cancel (back to Shell) |
| `^U` | Clear input |
| `^T` | Cancel (back to Shell) |

### Review mode

| Key | Action |
|-----|--------|
| `Enter` / `y` | Approve proposal |
| `Esc` / `n` | Cancel proposal |
| `^Q` | Quit |

### Run selector

| Key | Action |
|-----|--------|
| `↑↓` / `jk` | Navigate |
| `Enter` | Select session |
| `1-9` | Jump to session |
| `^N` / `^W` | New / close session |
| `Esc` | Cancel |

### Raw Play mode

| Key | Action |
|-----|--------|
| `^\` (Ctrl+Backslash) | Exit Raw Play, return to Console |
| `^Q` | Exit Raw Play and quit |
| Everything else | Forwarded to the game/app |

## Modes

**Shell** — Default mode. Keystrokes go to your shell. Console chrome (status bar, footer hints) is visible.

**Ask** — Intent composer. Type what you want in natural language, press Enter. Console sends your intent to the local AI planner, which generates a command proposal.

**Review** — Shows the generated command, risk level, confidence score, safety flags, and explanation. Approve to execute, cancel to go back and refine.

**Runs** — Run selector overlay. See all sessions with state badges (IDLE, RUN, BOOT, STOP, DONE, ERR!), unread markers, and CWD. Switch, create, or close sessions.

**Raw Play** — Fullscreen passthrough. Console exits its alternate screen, gives the host terminal directly to the PTY. The game/app gets real terminal behavior — ncurses, escape sequences, resize events, everything. Only `^\` returns to Console. Transcript is captured but visually degraded (raw escape sequences stored as-is).

## Architecture

Console is one of two shells in the CommandUI product family. Both shells are adapters over the same shared crates:

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

- **Transcript degraded in Raw Play** — output is captured but stored with raw escape sequences, not formatted for replay
- **No UI during Raw Play** — no split-screen, no real-time AI overlay. Exit Raw Play first to use Console features
- **Session switching blocked during Raw Play** — must exit with `^\` first
- **No workflow UI, memory UI, or history reopen yet** — these exist in the Desktop shell but are deferred for Console
- **Console cannot detect child process exit** — if a fullscreen app quits, you may need to press `^\` manually

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
