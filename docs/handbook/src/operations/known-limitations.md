# Known Limitations

## Shell and Terminal

### Completion detection
Command completion relies on prompt-marker injection. If your shell prompt is heavily customized or strips injected markers, completion detection may fail. The terminal may enter a `desynced` state — use the Resync button to recover.

### Exit code fidelity
Exit codes are extracted from shell markers. Some shells or commands may not report exit codes accurately through this mechanism.

### PTY session restore
Sessions are ephemeral. The PTY process is respawned on app restart. Terminal output and shell state (environment variables, aliases) do not persist across restarts. History, workflows, and memory do persist.

### Direct terminal typing
Keystrokes typed directly into the terminal (bypassing the composer) are not tracked in structured history. Only commands submitted through the composer are recorded.

## Planner

### Mock fallback
When Ollama is unavailable, the planner returns stub responses. The mock planner recognizes a few common intents (git status, destructive commands) but otherwise echoes the intent text. The "Mock planner — Ollama not connected" notice appears in the plan panel.

### Context window
The planner context includes up to 5 recent commands, 5 relevant workflows, and all effective memory items. Very large memory sets or workflow libraries may need pruning.

### No multi-turn conversation
The planner generates a single command plan per request. It does not support follow-up questions, clarifications, or multi-step reasoning. Each request is independent.

## UX

### No tab reordering
Session tabs cannot be reordered by dragging. They appear in creation order.

### No workflow editing
Saved workflows cannot be edited after creation (except through the promotion editor during initial save). To modify a workflow, delete it and recreate it.

### No memory editing
Memory items can be viewed and deleted, but not edited. To change a memory item, delete it and let the detectors regenerate a new suggestion (or accept a manual one).

### Keyboard shortcuts not customizable
All shortcuts are hardcoded. They use Ctrl-based combos. Custom keybindings are not supported.

### Terminal theme
The terminal uses a hardcoded dark theme. Light mode and custom themes are not yet available.

### No terminal search
Scrollback search (Ctrl+F in the terminal) is not implemented.

## Platform

### Windows
PowerShell 7 (`pwsh`) is preferred. Legacy `powershell.exe` works but may have quirks with marker detection. Set `COMMANDUI_WINDOWS_SHELL` environment variable to override.

### macOS / Linux
Reads `$SHELL` environment variable. Tested with bash and zsh.

### ARM
No ARM-specific testing has been done. The app should work on Apple Silicon via Rosetta but has not been verified natively.

## Data

### No export
History, workflows, and memory cannot be exported to external formats.

### No sync
All data is local. There is no cloud sync, team sharing, or multi-device support.

### No backup
The SQLite database is not automatically backed up. Manual backup by copying the database file from the app data directory.
