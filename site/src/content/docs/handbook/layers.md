---
title: System Layers
description: Six-layer architecture overview for CommandUI covering shell host, frontend, PTY, planner, storage, and memory.
sidebar:
  order: 13
---

CommandUI is a six-layer system. Each layer has a clear responsibility and communicates through defined contracts.

## Layer 1: Desktop Shell Host (Tauri)

The Rust backend. Manages:

- App lifecycle and window management
- PTY session creation and management (via `portable-pty`)
- SQLite persistence (via `rusqlite`)
- Ollama LLM integration (via `reqwest`)
- IPC bridge — exposes Tauri commands that the frontend calls via `invoke()`

**Key files:**
- `apps/desktop/src-tauri/src/main.rs` — app entry point, command registration
- `apps/desktop/src-tauri/src/commands/` — Tauri command handlers (session, terminal, planner, history, etc.)
- `apps/desktop/src-tauri/src/ollama.rs` — Ollama API client and prompt builder
- `apps/desktop/src-tauri/src/state.rs` — shared app state

## Layer 2: Frontend (React 19)

The user interface. Renders:

- Terminal pane (xterm.js)
- Input composer with mode toggle
- Plan panel with review/edit/approve flow
- Drawers: history, workflows, memory, settings
- Session tabs
- Command palette

**Key files:**
- `apps/desktop/src/app/AppShell.tsx` — central orchestrator (all state coordination)
- `apps/desktop/src/components/` — UI components
- `apps/desktop/src/hooks/` — custom hooks (shortcuts)

## Layer 3: PTY / Execution

Creates shell sessions, streams stdout/stderr, runs approved commands, tracks exit codes, preserves cwd. The execution layer never cares if a command came from raw input or AI — it executes the same way.

**Abstractions:** `ShellSession`, `ExecutionRequest`, `ExecutionResult`

**Event flow:** command submitted → `terminal:execution_started` → `terminal:line` (output) → `terminal:execution_finished` (with exit code and status)

## Layer 4: Planner Pipeline

Translates natural language intent into command plans:

1. Context assembly — gathers cwd, recent commands, memory, workflows
2. LLM generation — Ollama produces a structured plan (or mock fallback)
3. Safety annotation — flags destructive/network/privilege operations
4. UI presentation — plan panel shows the result for user review

**Key contract:** model output never executes directly. The user always approves.

## Layer 5: Local Data (SQLite)

Persistent storage for:

- Sessions (metadata, not PTY state)
- Commands and execution results
- Plans (generated command plans)
- History items
- Memory items and suggestions
- Workflows
- Settings

SQLite database is auto-created on first launch. Location varies by OS.

## Layer 6: Memory

Narrow, transparent preference learning:

- Pattern detectors observe history
- Suggestions surface when patterns reach confidence thresholds
- Accepted suggestions become memory items
- Memory items feed the planner context
- All items visible, editable, deletable

## Dependency direction

```
UI Components
    ↓
Zustand Stores (@commandui/state)
    ↓
API Contract (@commandui/api-contract)
    ↓
Domain Types (@commandui/domain)
```

Packages depend downward only. Domain types have zero dependencies. State depends on domain. API contract depends on domain. Components depend on everything above.

## Monorepo structure

```
commandui/
  apps/desktop/          — Tauri + React app
    src/                 — React frontend
    src-tauri/           — Rust backend
  packages/
    domain/              — Pure TypeScript types
    api-contract/        — Request/response contracts
    state/               — Zustand stores
    ui/                  — Shared UI primitives (future)
```
