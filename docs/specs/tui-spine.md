# CommandUI Console — Doctrine

**Product:** CommandUI Console (`commandui-console`)
**Sibling:** CommandUI Desktop (`commandui-desktop`)
**Status:** Phase 0 — doctrine locked, extraction not started

---

## Thesis

CommandUI is one product with two shells.

Desktop is the spatial shell — richer layout, visual review, multi-panel composition.
Console is the focused shell — speed, keyboard-only operation, SSH/devbox use, zero GUI dependency.

Both shells consume the same runtime. Neither is primary. Neither is a port of the other.

---

## Non-Negotiables

These define what CommandUI is in any shell. If Console loses any of these, it has drifted and should be killed.

1. **Real shell session ownership.** Console hosts a live PTY. The user types directly into a real shell.
2. **Two equal modes.** Shell mode (direct terminal) and Intent mode (ask → generate → review → approve) are both first-class. Neither is bolted on.
3. **Review before execution.** Generated commands are shown, explained, and approved before they run. Risk tier is visible.
4. **Risk-tiered confirmation.** Low-risk commands can auto-execute (if the user opts in). Medium and high require explicit approval. This is a product law, not a UI preference.
5. **History with rerun.** Every executed command is stored. Any entry can be rerun or its plan reopened.
6. **Local-first persistence.** SQLite. Sessions, history, plans, workflows, memory — all local.
7. **Multi-session support.** Even if Console v1 ships with a simple switcher, the runtime must support multiple concurrent sessions.

---

## What May Differ Between Shells

These are explicitly allowed to diverge:

- Layout and navigation (panels vs overlays vs splits)
- Settings surface depth
- Workflow editor complexity
- Memory management UI richness
- Mouse support (Console may have none)
- Visual flourish and animation
- Plugin/extension model (if Desktop ever gets one)

These must NOT diverge:

- Session lifecycle semantics
- Execution state machine
- Event contracts
- Risk classification logic
- History/plan storage format
- Planner request/response contract

---

## Architecture Law: One Runtime, Two Adapters

```
┌─────────────────────────────────────────────┐
│              Runtime Core                    │
│  PTY lifecycle · session registry · events  │
│  exec state machine · persistence · planner │
└──────────────┬──────────────┬───────────────┘
               │              │
      ┌────────┴───┐   ┌─────┴────────┐
      │  Desktop   │   │   Console    │
      │  Adapter   │   │   Adapter    │
      │  (Tauri)   │   │  (Ratatui)   │
      └────────────┘   └──────────────┘
```

**Runtime owns truth.** The runtime:
- Spawns and manages PTY sessions
- Runs the reader loop
- Parses prompt markers
- Drives the execution state machine
- Produces semantic events
- Persists history, plans, workflows, memory, settings

**Adapters translate.** An adapter:
- Receives runtime events and renders them for its shell
- Captures user input and translates it into runtime service calls
- Owns its own layout, navigation, and input routing
- Never reimplements runtime behavior

**The boundary is `RuntimeEventSink`.** The runtime pushes events through a trait. Each adapter implements the trait. That is the only integration seam.

---

## First-Class Design Problems

### 1. Input Ownership

Console hosts a live PTY inside a TUI app. Every keystroke must go to exactly one consumer. The mode model must be explicit and unambiguous.

**Modes:**

| Mode | Keys go to | Entered via | Exited via |
|------|-----------|-------------|------------|
| Shell | PTY (live terminal) | Default, or leave other modes | Enter Composer, open overlay |
| Composer | App input bar | Hotkey from Shell mode | Submit, cancel, or switch to Shell |
| Review | App review panel | Plan generated (auto-enter) | Approve, reject, edit, cancel |
| Overlay | App overlay (history, sessions, settings) | Hotkey | Dismiss, select |

**Laws:**
- There is always exactly one active mode.
- Mode transitions are explicit — never implicit or inferred.
- The current mode is always visible in the UI (status bar badge at minimum).
- Shell mode is the gravity well. If in doubt, return to Shell.
- No keystroke is silently swallowed. If a key does nothing in the current mode, it is either ignored visibly or the mode is wrong.

**Fullscreen child programs (vim, htop, less):**
Console v1 policy: when the PTY is in Shell mode, all keys go through. The app chrome becomes minimal (status bar only). Overlays and Composer are still accessible via a designated escape chord (e.g., Ctrl+Space). This is "transparent passthrough" — the TUI steps aside rather than trying to interpret what the child is doing.

### 2. Resize Ownership

Console runs inside a host terminal. When the host resizes, two things must update:
1. The app chrome layout (composer height, pane splits, status bar)
2. The embedded PTY viewport (rows × cols)

**Resize ownership law:** Host resize is first interpreted by the shell chrome. The PTY is resized only after the terminal viewport dimensions are re-derived from the new layout.

**Sequence:**
1. Host terminal emits SIGWINCH (or equivalent)
2. Ratatui receives new terminal size
3. App layout engine recomputes all pane dimensions
4. Terminal pane's new rows/cols are determined
5. PTY is resized to match
6. Redraw on new geometry

**Coalescing:** Resize events must be debounced. Drag-resizing fires a burst of signals. Both the layout engine and PTY must tolerate rapid updates without thrashing, cursor drift, or stale line wraps.

**Mode interaction:** If the user is in Review or Overlay mode during a resize, the layout engine must still recompute the terminal pane size. The PTY resize happens regardless of app mode, because the shell process doesn't know about modes.

### 3. Fullscreen Passthrough

When the child process is a TUI (vim, htop, a nested Ratatui app):
- Console cannot interpret the PTY output stream as "lines" in the normal sense
- The terminal pane must act as a raw viewport
- App chrome must shrink to minimum (status bar only)
- Console does not attempt to detect whether the child is fullscreen — it simply passes through in Shell mode

This is not a v1 blocker but it must not be designed out. The architecture must tolerate raw PTY passthrough without special casing.

### 4. Cross-Shell Drift

The most insidious risk. Desktop and Console start with the same runtime, then slowly reimplement behavior in their adapters because "it was easier."

**Prevention:**
- Runtime is the only place execution state transitions happen
- Runtime is the only place events are produced
- Adapters never gate on raw PTY output — they consume semantic events only
- If a behavior needs to differ between shells, it must be a runtime-level configuration, not adapter-level reimplementation
- Test suite runs against the runtime service API, not through either adapter

---

## Console v1 Scope

### Ships in v1

- Single main shell view with live PTY
- Bottom composer bar with mode toggle (COMMAND / ASK)
- Plan review panel (inline or overlay)
- History overlay with rerun
- Session switcher (simple list, hotkey navigation)
- Status strip: cwd, session name, exec state, risk tier, current mode
- Risk badge + confirm step before execution
- Settings view (basic — mode defaults, model config)
- SQLite persistence (sessions, history, plans, settings)

### Explicit non-goals for v1

- Full parity with Desktop's drawer/panel system
- Workflow editor
- Memory management UI (memory persists, but management is Desktop's job for now)
- Mouse support
- Plugin/extension system
- Remote/multi-user sync
- Command palette
- Themes beyond light/dark
- Markdown rendering in plan review (plain text is fine)

---

## Technology

- **Runtime:** Rust (shared crates)
- **TUI framework:** Ratatui
- **PTY:** Same `portable-pty` used by Desktop
- **Persistence:** Same SQLite schema
- **Planner:** Same Ollama integration (Phase 2 extraction)
- **No Python, no Electron, no web runtime**

Single language, single runtime, one event model, one concurrency model, one persistence stack.

---

## Extraction Phases

The runtime extraction plan lives in the [extraction worksheet](./tui-extraction-worksheet.md). Summary:

| Phase | Name | What |
|-------|------|------|
| 0 | Doctrine | This document + extraction worksheet |
| 1A | Runtime truth surface | Neutral event types, move pure modules to `crates/runtime-core` |
| 1B | Persistence services | Extract CRUD from Tauri commands to `crates/runtime-persistence` |
| 1C | Event sink | `RuntimeEventSink` trait, refactor reader loop |
| 1D | Session/terminal services | Session lifecycle + terminal execution behind runtime service |
| 2 | Planner cleanup | Move planner types to runtime, invert ollama dependency |
| 3 | Console shell | Build Ratatui app consuming runtime services |

1A and 1B are independent (can run in parallel).
1C depends on 1A.
1D depends on 1C.
Phase 2 can begin type moves after 1A but full cleanup waits.
Phase 3 begins after 1D proves out.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-23 | Product name: CommandUI Console | "Console" implies control surface, not just raw shell. Lands faster than "Terminal" for the product thesis. |
| 2026-03-23 | Ratatui over Python Textual | One runtime, one PTY owner, no IPC seam at the hardest boundary. |
| 2026-03-23 | Planner extraction deferred to Phase 2 | Functionally loose-coupled. Ugly dependency direction but not load-bearing for shell/session/event truth. |
| 2026-03-23 | Resize ownership promoted to doctrine | TUI has two consumers of one resize signal (chrome + PTY). Sequencing matters. Not a detail — a law. |
| 2026-03-23 | Input ownership: 4 explicit modes | Shell, Composer, Review, Overlay. No implicit transitions. Mode always visible. |
