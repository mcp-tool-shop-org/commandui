---
title: Browser Preview and Mock Bridge
description: How the mock bridge simulates the Tauri backend for browser-based development and testing.
sidebar:
  order: 22
---

When you run `pnpm dev`, CommandUI starts in browser preview mode. The mock bridge replaces all Tauri backend calls with in-memory simulations.

## How it works

The mock bridge intercepts all `invoke()` calls that would normally go to the Rust backend:

1. `isTauriRuntime()` checks if `window.__TAURI_INTERNALS__` exists
2. If not (browser), `tauriInvoke()` routes to `mockInvoke()` instead
3. `mockInvoke()` dispatches to handler functions that simulate backend behavior
4. Events use `CustomEvent` with `mock:` prefix instead of Tauri's event system

## Mock command handlers

| Command | Simulation |
|---------|------------|
| `session_create` | Creates session with UUID, emits ready event after 100ms |
| `session_list` | Returns in-memory session list |
| `session_close` | Removes session from memory |
| `terminal_execute` | Simulates PTY with staggered output lines based on command |
| `terminal_interrupt` | Marks running execution as interrupted |
| `terminal_resync` | Transitions to ready state after 200ms |
| `terminal_resize` | No-op |
| `terminal_write` | No-op |
| `planner_generate_plan` | Returns mock plan; fuzzy-matches against known workflows |
| `history_append/list/update` | CRUD on in-memory array |
| `workflow_add/list/delete` | CRUD on in-memory array |
| `settings_get` | Returns guided mode defaults |
| `settings_update` | No-op |
| `memory_*` | Full memory simulation (items, suggestions, accept, dismiss, delete) |
| `plan_store` | No-op |

## Mock terminal output

The mock bridge provides plausible output for common commands:

| Input | Mock output |
|-------|-------------|
| `echo <text>` | Returns the text |
| `ls` / `dir` | File listing (README.md, src/, package.json, etc.) |
| `pwd` / `cd` | Returns `~/projects` |
| `git status` | "On branch main / nothing to commit" |
| `git log` | Two mock commit entries |
| Other | `[mock] <command> / (browser preview — command not executed)` |

## Event simulation

Mock events are emitted as `CustomEvent` on `window`:

```javascript
window.dispatchEvent(new CustomEvent("mock:terminal:line", {
  detail: { sessionId, text }
}));
```

The `onMockEvent()` wrapper subscribes to these:

```typescript
function onMockEvent(eventName: string, callback: (payload) => void): () => void
```

Returns an unsubscribe function, matching the Tauri event API contract.

## Workflow-aware mock planner

The mock bridge's `planner_generate_plan` handler is workflow-aware:

1. Extracts `projectFacts` from the request context
2. Finds workflow facts (where `kind === "workflow"`)
3. Fuzzy-matches the user's intent against workflow labels
4. If matched, returns the workflow's commands instead of a generic echo stub

This lets you test the full workflow → planner feedback loop in browser preview.

## When to use browser preview

- UI component development and styling
- Testing user flows (history, workflows, memory)
- Keyboard shortcut testing
- Layout and responsive behavior
- Verifying state management logic

## When you need Tauri

- Real shell execution
- PTY behavior (stdout/stderr, exit codes, cwd tracking)
- SQLite persistence
- Ollama LLM integration
- Platform-specific testing (window management, system tray)
