# Known Limitations

## Shell Completion Detection
- A prompt hook reports each prompt with an invisible OSC 7733 sequence (nonce, exit code, cwd)
- Supports: PowerShell, pwsh, bash (Git Bash on Windows too), zsh, cmd
- A prompt framework that replaces the hook after startup stops detection
- Interactive commands (vim, htop) show as "running" until exit

## Exit Code Fidelity
- PowerShell uses `$?` and `$LASTEXITCODE`, bash/zsh use `$?`
- cmd: an approved command captures `%ERRORLEVEL%` in a hidden tail; a hand-typed cmd command reports completion but no exit code

## PTY Session Restore
- Sessions are ephemeral per app launch
- Shell processes don't survive app restart
- Session metadata restored, but PTY must be respawned

## Planner
- Calls a local Ollama model first. The prompt includes the working directory, recent commands, known preferences, and known workflows.
- If the model is not installed, not running, or not downloaded, Ask says so and does not draft a command in a release build. A debug build can show a labeled practice plan.
- Context is the current session, not a long transcript.

## Semantic Review
- Edit-and-run works, but original plan metadata not fully preserved on reopen
- Reopened plans are synthetic reconstructions from history
- No plan diffing or version tracking

## UX
- No tab reordering or renaming
- A saved workflow can be edited. Save Workflow on one command does not open the editor
- No memory editing (only delete + re-accept)
- Keyboard shortcuts are Ctrl-based (no customization). Ask is Ctrl+Shift+A, not Ctrl+2
- The window and the terminal follow the system light or dark scheme. There is no theme setting

## Platform
- Windows: pwsh 7 preferred, fallback to powershell.exe
- macOS/Linux: reads SHELL env, fallback to /bin/bash
- No ARM-specific testing yet
