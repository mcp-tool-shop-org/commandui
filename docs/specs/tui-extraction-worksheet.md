# CommandUI Runtime Extraction Worksheet

**Source:** `apps/desktop/src-tauri/src/` — 20 files, ~2,868 LOC
**Companion:** [tui-spine.md](./tui-spine.md) (doctrine)
**Status:** Phase 0 — audit complete, extraction not started

---

## Tags

| Tag | Meaning | Phase |
|-----|---------|-------|
| **MOVE** | Move as-is into runtime crate | 1A |
| **WRAP** | Extract service method, leave thin Tauri wrapper | 1B |
| **SINK** | Requires event sink abstraction first | 1C |
| **REWRITE** | Rewrite around runtime service + event sink | 1D |
| **DEFER** | Phase 2 (planner cleanup) | 2 |
| **KEEP** | Stays in desktop adapter | — |

---

## File-by-File Extraction Map

### Pure Domain — MOVE (Phase 1A)

| File | LOC | Action | Notes |
|------|-----|--------|-------|
| `shell/pty_manager.rs` | 122 | **MOVE** → `crates/runtime-core/src/pty.rs` | Pure. No Tauri imports. `spawn_reader_loop` closure signature changes from `Fn(String)` to channel-based event production. PTY spawn/write/bootstrap logic is clean. |
| `shell/session_registry.rs` | 150 | **MOVE** → `crates/runtime-core/src/session.rs` | Pure. `SessionExecState`, `SessionRecord`, all registry CRUD. Zero Tauri deps. |
| `db/sqlite.rs` | 6 | **MOVE** → `crates/runtime-persistence/src/db.rs` | Pure. `open_database(path) -> Connection`. |
| `db/schema.rs` | 122 | **MOVE** → `crates/runtime-persistence/src/schema.rs` | Pure. 6 tables, idempotent migrations. Shell-agnostic. |
| `types/errors.rs` | 72 | **MOVE** → `crates/runtime-core/src/errors.rs` | Pure. `ErrorCode` + `ApiError`. Already shell-agnostic. |

**Also in 1A — extract event contracts:**

These structs currently live inside `commands/session.rs` and `commands/terminal.rs`. Move to `crates/runtime-core/src/events.rs`:

| Event struct | Current location | Runtime event |
|-------------|-----------------|---------------|
| `TerminalLineEvent` | session.rs | `RuntimeEvent::TerminalLine` |
| `SessionReadyEvent` | session.rs | `RuntimeEvent::SessionReady` |
| `SessionCwdChangedEvent` | session.rs | `RuntimeEvent::CwdChanged` |
| `ExecutionFinishedEvent` | session.rs | `RuntimeEvent::ExecutionFinished` |
| `SessionExecStateChangedEvent` | session.rs + terminal.rs | `RuntimeEvent::ExecStateChanged` |
| `ExecutionStartedEvent` | terminal.rs | `RuntimeEvent::ExecutionStarted` |
| `ExecutionSummary` | terminal.rs | (payload inside `ExecutionStarted`) |

**1A deliverable:** `crates/runtime-core` compiles independently. All 6 event types have a canonical home. Both shells will import from here.

---

### Persistence Services — WRAP (Phase 1B)

| File | LOC | Action | Notes |
|------|-----|--------|-------|
| `commands/history.rs` | 212 | **WRAP** → `crates/runtime-persistence/src/history.rs` | 4 commands (`append`, `list`, `update`, `plan_store`). All DB CRUD. No AppHandle, no events. Extract service methods, leave 3-line Tauri wrappers. |
| `commands/settings.rs` | 130 | **WRAP** → `crates/runtime-persistence/src/settings.rs` | 2 commands (`get`, `update`). DB ops + JSON merge. Same pattern. |
| `commands/workflow.rs` | 115 | **WRAP** → `crates/runtime-persistence/src/workflow.rs` | 3 commands (`add`, `list`, `delete`). Trivial DB CRUD. |
| `commands/memory.rs` | 310 | **WRAP** → `crates/runtime-persistence/src/memory.rs` | 6 commands. All DB CRUD. `accept_suggestion` has a read+insert+update flow but still pure persistence. |

**1B deliverable:** `crates/runtime-persistence` owns all DB access. Tauri command files shrink to thin wrappers that unpack request → call service → return response. 1B is independent of 1A and can run in parallel.

---

### Event Sink — SINK (Phase 1C)

**Depends on:** 1A (event types must exist)

**New file:** `crates/runtime-core/src/sink.rs`

```rust
pub trait RuntimeEventSink: Send + Sync + 'static {
    fn emit(&self, event: RuntimeEvent);
}
```

**Impact:** The reader loop in `session_create` currently captures `AppHandle` and calls `app.emit()` directly for 5 event types. After 1C:

- Reader loop captures `Arc<dyn RuntimeEventSink>` instead of `AppHandle`
- Reader loop produces `RuntimeEvent` variants instead of calling `app.emit("session:ready", ...)`
- Desktop adapter implements `RuntimeEventSink` by translating to Tauri events
- Console adapter (Phase 3) implements it by sending to Ratatui's event channel

**Also affected:**
- `emit_exec_state_changed()` helper in session.rs → becomes `sink.emit(RuntimeEvent::ExecStateChanged(...))`
- `emit_exec_state()` helper in terminal.rs → same (deduplicated — one helper, one location)

**1C deliverable:** Reader loop is runtime-owned. No Tauri imports in the reader path. Event production is adapter-agnostic.

---

### Session/Terminal Services — REWRITE (Phase 1D)

**Depends on:** 1C (event sink must exist)

| File | LOC | Action | Notes |
|------|-----|--------|-------|
| `commands/session.rs` | 384 | **REWRITE** | Split into: (1) `RuntimeSessionService::create()` — spawns PTY, starts reader loop with sink, returns session info. (2) `RuntimeSessionService::list()` / `close()` / `update_cwd()` — trivial, can WRAP early. (3) Reader loop becomes a runtime-owned background task. |
| `commands/terminal.rs` | 292 | **REWRITE** | Split into: (1) `RuntimeTerminalService::execute()` — validate state, write PTY, transition state, emit events via sink. (2) `interrupt()` / `resync()` — same pattern. (3) `resize()` / `write()` — already clean, can WRAP early. Fix: double-lock in `terminal_execute` becomes single lock in service method. |
| `state.rs` | 35 | **REWRITE** | `AppState` becomes `RuntimeServices` (sessions + persistence + config). Desktop gets `DesktopState` wrapping runtime + `AppHandle`. Console gets `ConsoleState` wrapping runtime + Ratatui sender. |

**Early wins within 1D:** These functions inside the REWRITE files don't need the sink and can wrap immediately:

| Function | Why it's easy |
|----------|--------------|
| `session_list` | Read-only registry query |
| `session_close` | Registry remove, no events |
| `session_update_cwd` | Registry mutate, no events |
| `terminal_resize` | PTY resize call, no events |
| `terminal_write` | Raw PTY write, no events |

**1D deliverable:** Session lifecycle and terminal execution are runtime service calls. Desktop adapter is a thin translation layer. The runtime is TUI-ready.

---

### Planner Cleanup — DEFER (Phase 2)

| File | LOC | Action | Notes |
|------|-----|--------|-------|
| `commands/planner.rs` | 352 | **DEFER** | Takes `State<AppState>` but no AppHandle, no events. Mock fallback is ugly but harmless. Extract `PlannerContext`, `ProjectFact`, `MemoryItemSummary`, `CommandPlanPayload`, `PlanReviewPayload` to `crates/runtime-core/src/planner_types.rs`. Leave command as thin wrapper. |
| `ollama.rs` | 481 | **DEFER** | Functionally pure (reqwest + serde). Bad import: `use crate::commands::planner::PlannerContext`. After type extraction to runtime-core, this inverts cleanly → `crates/runtime-planner/src/ollama.rs`. |

**Phase 2 deliverable:** Planner types live in runtime. Ollama client depends on runtime, not on command modules. Dependency direction is correct.

---

### Desktop Adapter — KEEP

| File | LOC | Action | Notes |
|------|-----|--------|-------|
| `main.rs` | 56 | **KEEP** | Composition root. After extraction: construct `RuntimeServices`, wrap in `DesktopState`, register Tauri handlers. Shrinks significantly. |
| `lib.rs` | 7 | **KEEP** | Module root. Re-exports adapter layer. |
| `commands/mod.rs` | 18 | **KEEP** | Tauri command registry. |

---

## Target Crate Structure

```
commandui/
  crates/
    runtime-core/
      src/
        lib.rs
        pty.rs              ← from shell/pty_manager.rs
        session.rs          ← from shell/session_registry.rs
        events.rs           ← extracted from commands/session.rs + terminal.rs
        errors.rs           ← from types/errors.rs
        sink.rs             ← RuntimeEventSink trait (Phase 1C)
        service.rs          ← RuntimeSessionService + RuntimeTerminalService (Phase 1D)
        planner_types.rs    ← from commands/planner.rs types (Phase 2)
    runtime-persistence/
      src/
        lib.rs
        db.rs               ← from db/sqlite.rs
        schema.rs           ← from db/schema.rs
        history.rs          ← from commands/history.rs
        settings.rs         ← from commands/settings.rs
        workflow.rs         ← from commands/workflow.rs
        memory.rs           ← from commands/memory.rs
    runtime-planner/        ← Phase 2
      src/
        lib.rs
        ollama.rs           ← from ollama.rs
  apps/
    desktop/                ← existing, thins out
      src-tauri/src/
        main.rs             ← composition root
        lib.rs
        state.rs            ← DesktopState (wraps RuntimeServices + AppHandle)
        commands/            ← thin Tauri wrappers only
    console/                ← Phase 3
      src/
        main.rs
        app.rs              ← Ratatui app + event loop
        state.rs            ← ConsoleState (wraps RuntimeServices + channel sender)
        sink.rs             ← ConsoleEventSink impl
        ui/                 ← layout, panes, overlays
        input.rs            ← mode router
```

---

## Smoke Tests Per Phase

| Phase | Verification |
|-------|-------------|
| 1A | `cargo check -p runtime-core` passes. Event types compile. |
| 1B | `cargo check -p runtime-persistence` passes. Desktop still builds and runs. Existing Tauri commands still work (wrappers call service methods). |
| 1C | Reader loop uses `Arc<dyn RuntimeEventSink>`. Desktop adapter implements sink. Desktop still emits all 6 event types correctly. Manual test: create session, run command, verify events in frontend. |
| 1D | `RuntimeSessionService::create()` and `RuntimeTerminalService::execute()` exist. Desktop calls them through adapter. Manual test: full create → execute → history → rerun flow still works. |
| 2 | `cargo check -p runtime-planner` passes. `ollama.rs` imports from `runtime-core`, not from `commands::planner`. |

---

## Known Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Reader loop extraction breaks event timing | High | Keep reader loop on spawned thread (same as today). Only change: closure captures sink instead of AppHandle. Test by running Desktop after 1C. |
| Double-lock in terminal_execute causes deadlock during rewrite | Medium | Service method holds one lock for full validate→write→transition→emit. Explicit test: rapid command submission. |
| Event struct changes break Desktop frontend | Medium | Keep Tauri event names + payload shapes identical. Desktop adapter translates RuntimeEvent → same JSON the frontend expects. |
| Planner type extraction introduces circular deps | Low | runtime-core defines types. runtime-planner depends on runtime-core. Commands depend on both. No cycles. |
| Resize sequencing wrong in Console | High (Console only) | Doctrine: chrome reflows first, PTY second. Debounce. Test with rapid drag-resize. |
| Input mode confusion in Console | High (Console only) | Doctrine: 4 explicit modes, always visible, Shell is gravity well. Test: every mode transition has a corresponding status bar update. |
