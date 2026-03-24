# Repo Map

## Repository
- URL: https://github.com/mcp-tool-shop-org/CommandUI
- Local: F:/AI/CommandUI
- Branch: main
- Version: v1.0.0 (desktop), v0.1.0 (console + crates)

## Language / Stack
Rust workspace. Tauri v2 + React (desktop), Ratatui + crossterm + tokio (console). SQLite via rusqlite. Ollama for local LLM inference.

## Directory Structure
- `crates/runtime-core/` — PTY, sessions, events, RuntimeEventSink trait, services (22 tests)
- `crates/runtime-persistence/` — SQLite CRUD: history, settings, workflows, memory (2 tests)
- `crates/runtime-planner/` — Ollama client, proposals, validation, mock fallback (37 tests)
- `apps/console/` — Ratatui TUI shell (36 tests)
- `apps/desktop/src-tauri/` — Tauri backend adapter (4 tests)
- `apps/desktop/src/` — React frontend
- `docs/specs/` — 4 doctrine specs (tui-spine, parity-law, play-law, raw-play-mode)

## Conventions
- Shared crates define product law; shell apps are adapters
- Events route by session_id — never by active index
- Proposals are session-bound — approval targets the proposal's session
- Parity tests prevent cross-shell drift
- InputMode enum is exhaustive — every key has exactly one owner

## Validation Law (Non-Negotiable)

This is a terminal-native product. Validation follows terminal truth, not browser truth.

**What validation means here:**
- `cargo check --workspace` — type-checks
- `cargo test --workspace` — 101 tests, including parity drift gates
- `cargo run -p commandui-console` — manual interactive launch for UX verification
- Code-path analysis for state machine correctness (mode transitions, session targeting, raw play lifecycle)

**What validation does NOT mean here:**
- No browser preview — Console is a TUI, not a web app
- No `preview_start`, `preview_screenshot`, `preview_snapshot`, or any `mcp__Claude_Preview__*` tool
- No desktop-style visual verification — no screenshots, no pixel diffing, no layout inspection
- No visual-polish verification path — function is verified by tests and manual terminal interaction
- No GUI-style acceptance criteria — "looks right" is not a valid check

**Raw play lifecycle is correctness, not UI:**
- Alternate screen enter/exit is a state machine invariant, not a rendering detail
- Terminal restoration after raw play is a correctness requirement, not polish
- Session-switching prohibition during raw play is a safety invariant, not a UX preference
- stdout routing discipline (active session only) is data integrity, not display logic

## Raw Play — First-Class Architecture Seam

Raw play mode is not a feature bolted onto Console. It is an architectural boundary where Console surrenders terminal ownership to a child process. Every invariant here is load-bearing.

### Alternate screen law
- **Enter raw play:** Console exits Ratatui's alternate screen (`LeaveAlternateScreen`), clears host terminal, shows entry hint, resizes PTY to full host dimensions. `in_raw_passthrough` flag set true.
- **Exit raw play:** Console re-enters alternate screen (`EnterAlternateScreen`), hides cursor, forces full redraw (`terminal.clear()`), resizes PTY back to Console pane dimensions. `in_raw_passthrough` flag set false.
- **Both transitions are idempotent** — calling enter when already in, or exit when already out, is a no-op.

### Session ownership during raw play
- **Active session only** — PTY output from the active session writes to host stdout. Non-active session output is NOT written to stdout (targeting truth).
- **Session switching is blocked** — `can_switch_sessions()` returns false during `InputMode::RawPlay`. Must exit raw play first.
- **Background sessions continue** — events still route to non-active sessions. Unread markers still set. Model state still updated. Only stdout output is gated.

### Quit path guarantees
- **Ctrl+Q during raw play** — exits raw mode first (restores alternate screen), then quits. Terminal is always left clean.
- **Session death during raw play** — `raw_play_tick()` checks `active_session_alive()` and auto-exits to Shell mode if the session dies. Terminal restored.
- **App cleanup on any exit** — `App::run()` checks `in_raw_passthrough` and restores alternate screen before `disable_raw_mode()` and `LeaveAlternateScreen`.

### Transcript law
- PTY output during raw play is still delivered as `TerminalLine` events and captured in the model.
- Transcript is captured but visually degraded — raw escape sequences stored as-is, not interpreted.
- Searchable but not replayable with visual fidelity.

### Escape chord
- `Ctrl+\` (SIGQUIT) — the only key Console intercepts during raw play (besides Ctrl+Q).
- Chosen because Ctrl+C, Ctrl+Z, Ctrl+Q are commonly captured by games/editors/shells.

### Key files for raw play:
- `apps/console/src/app.rs:334-380` — `enter_raw_play()`, `exit_raw_play()`
- `apps/console/src/app.rs:239-290` — `raw_play_tick()` event loop
- `apps/console/src/input.rs:282-324` — `handle_raw_play_key()`
- `apps/console/src/model.rs:296-305` — `can_switch_sessions()`, `active_session_alive()`

## Build / Run
```bash
cargo test --workspace          # All 101 tests
cargo test -p commandui-console # Console only (36 tests)
cargo run -p commandui-console  # Launch Console
cargo check --workspace         # Type-check everything
```

## Key Files
- `apps/console/src/model.rs` — Multi-session model, targeting truth
- `apps/console/src/input.rs` — 5-mode key router
- `apps/console/src/app.rs` — Event loop, raw play lifecycle
- `apps/console/src/ui.rs` — All rendering
- `crates/runtime-core/src/events.rs` — RuntimeEvent enum, RuntimeEventSink trait
- `crates/runtime-core/src/session.rs` — SessionRegistry, SessionExecState
- `crates/runtime-planner/src/types.rs` — CommandProposal, PlanContext

## Dependencies
- `portable-pty` — PTY spawning (cross-platform)
- `ratatui` + `crossterm` — TUI rendering + terminal events
- `tokio` — Async runtime for event loop + planner
- `reqwest` — Ollama HTTP client
- `rusqlite` (bundled) — Local SQLite persistence
- `serde` / `serde_json` — Serialization throughout

## Risky Seams
- PTY reader → event emitter: escape sequence parsing is raw, prompt markers fragile
- Raw play enter/exit: alternate screen toggle, PTY resize, cursor state — must be idempotent (see Raw Play section above)
- Session death during raw play: auto-exit must restore Console cleanly
- Proposal session binding: must survive active session changes during Review mode
- stdout routing: only active session writes to host stdout during raw play — violation is data corruption
