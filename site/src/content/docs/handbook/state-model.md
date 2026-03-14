---
title: State Model
description: Zustand store inventory and data flow patterns for CommandUI's nine client-side stores.
sidebar:
  order: 14
---

CommandUI uses Zustand for client-side state management. Nine stores handle distinct concerns.

## Store inventory

### ComposerStore
```
inputValue: string
inputMode: "command" | "ask"
```
The composer's current text and mode. Set by the user, consumed by AppShell on submit.

### ExecutionStore
```
activeExecutionId: string | null
lastExecutionId: string | null
executionStatus: "idle" | "running" | "success" | "failure" | "interrupted"
sessionExecStates: Record<string, SessionExecState>
```
Tracks what's executing and the per-session execution state machine.

**SessionExecState:** `booting` → `ready` → `running` → `ready` (normal flow). Can enter `interrupting` (Ctrl+C sent) or `desynced` (state lost, needs resync).

### SessionStore
```
sessions: SessionSummary[]
activeSessionId: string | null
```
Session list and which tab is active.

### HistoryStore
```
items: HistoryItem[]
```
Append-only history. Loaded from SQLite on boot, updated on every command execution.

### MemoryStore
```
items: MemoryItem[]
suggestions: MemorySuggestion[]
```
Accepted memory items and pending suggestions from pattern detectors.

### SettingsStore
```
productMode: "classic" | "guided"
reducedClutter: boolean
simplifiedSummaries: boolean
confirmMediumRisk: boolean
defaultInputMode: "command" | "ask"
```
User preferences. Hydrated from backend on boot, written back on change.

### WorkflowStore
```
items: Workflow[]
```
Saved workflows. CRUD operations backed by SQLite.

### WorkflowRunStore
```
activeRun: WorkflowRun | null
lastRunByWorkflowId: Record<string, WorkflowRun>
```
Tracks the currently executing workflow run and the last completed run per workflow.

### FocusStore
```
currentZone: FocusZone | null
previousZone: FocusZone | null
```
Tracks which UI zone has focus for keyboard shortcut resolution. Zones: `composer`, `terminal`, `plan`, `drawer`, `palette`.

## Data flow

```
User Action → AppShell handler → Tauri invoke (backend)
                                      ↓
                              Backend processes + persists
                                      ↓
                              Tauri event emitted
                                      ↓
                              AppShell event listener
                                      ↓
                              Zustand store update → React re-render
```

## Coordination

AppShell is the central coordinator. It:
- Reads from all stores
- Dispatches actions to backends via Tauri invoke
- Listens for backend events and updates stores
- Passes store data to components as props

Components are mostly presentational — they receive data and callbacks, they don't directly invoke backends or manage cross-cutting state.
