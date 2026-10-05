# Known Limitations

## Shell and Terminal

### Completion detection
Command completion relies on a prompt hook CommandUI installs in the shell, which reports each prompt with an invisible escape sequence. A prompt framework that replaces that hook after startup stops completion detection; the session stays `running` until you interrupt or resync it.

### Exit code fidelity
Exit codes come from the shell itself: `$?` in bash and zsh, `$?` and `$LASTEXITCODE` in PowerShell, and `%ERRORLEVEL%` captured after each approved command in cmd. A command you type yourself in cmd reports no exit code, only that it finished.

### Sessions do not survive a restart
The shell starts again when you reopen CommandUI. Terminal output, environment variables, and aliases do not. History, workflows, and memory do.

### Direct terminal typing
Keystrokes typed directly into the terminal (bypassing the composer) are not tracked in structured history. Only commands submitted through the composer are recorded.

## Planner

### When the model is not ready
Ask says whether the model is not installed, not running, or not downloaded, and what to do next. A release build does not draft a command in that case. A debug build and the browser preview can show a practice plan, labeled "Practice plan — Ollama is not connected. This is not a real plan."

### Context window
The planner context includes up to 5 recent commands, 5 relevant workflows, and all effective memory items. Very large memory sets or workflow libraries may need pruning.

### No multi-turn conversation
The planner generates a single command plan per request. It does not support follow-up questions, clarifications, or multi-step reasoning. Each request is independent.

## UX

### No tab reordering
Session tabs cannot be reordered by dragging. They appear in creation order.

### Workflow editing
Edit on a saved workflow opens the editor. Save Workflow on a plan, or on a single history row, stores that one command and does not open the editor.

### No memory editing
Memory items can be viewed and deleted, but not edited. To change a memory item, delete it and let the detectors regenerate a new suggestion (or accept a manual one).

### Keyboard shortcuts not customizable
All shortcuts are hardcoded. They use Ctrl-based combos. Custom keybindings are not supported.

### Terminal theme
The window and the terminal follow the system light or dark scheme. There is no separate theme setting. Text size is 100% to 200%.

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
