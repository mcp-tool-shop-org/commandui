# Persistence

CommandUI uses SQLite for all persistent storage. The database is created automatically on first launch.

## Database location

| OS | Path |
|----|------|
| Windows | `%APPDATA%/com.commandui.desktop/` |
| macOS | `~/Library/Application Support/com.commandui.desktop/` |
| Linux | `~/.local/share/com.commandui.desktop/` |

## What persists

| Data | Persists? | Notes |
|------|-----------|-------|
| Sessions (metadata) | Yes | Session labels, creation time |
| PTY state | No | Shell process respawned on restart |
| Terminal output | No | Buffered in memory only |
| History | Yes | All structured command history |
| Plans | Yes | Generated command plans |
| Settings | Yes | User preferences |
| Memory items | Yes | Accepted pattern observations |
| Memory suggestions | Yes | Pending suggestions |
| Workflows | Yes | Saved command workflows |
| Workflow runs | No | Last-run tracked in memory only |

## Boot hydration sequence

On app launch, the frontend loads data from the backend in this order:

1. **Settings** — applied to SettingsStore (non-critical, swallowed errors)
2. **Sessions** — loaded or created if none exist
3. **History** — last 100 items loaded into HistoryStore
4. **Memory** — items and suggestions loaded into MemoryStore
5. **Suggestions** — pattern detectors re-run on loaded history
6. **Workflows** — loaded into WorkflowStore

Each step is wrapped in try-catch. Non-critical failures (settings, history, memory, workflows) are swallowed — the app boots in a degraded but functional state. Only session creation failure triggers a boot error.

## Backend API for persistence

| Command | Purpose |
|---------|---------|
| `history_append` | Add a new history item |
| `history_list` | Load recent history |
| `history_update` | Update an existing item (e.g., add exit code after execution) |
| `plan_store` | Save a generated plan |
| `workflow_add` | Save a new workflow |
| `workflow_list` | Load all workflows |
| `workflow_delete` | Remove a workflow |
| `memory_add` | Save a memory item |
| `memory_list` | Load all items and suggestions |
| `memory_accept_suggestion` | Convert suggestion to accepted item |
| `memory_dismiss_suggestion` | Mark suggestion as dismissed |
| `memory_delete` | Remove a memory item |
| `settings_get` | Load settings |
| `settings_update` | Save settings changes |
