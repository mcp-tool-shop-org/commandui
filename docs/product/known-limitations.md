# Known Limitations

## Shell Completion Detection
- Based on prompt-marker injection (`__COMMANDUI_PROMPT__`)
- Supports: PowerShell, pwsh, bash, zsh, cmd
- If user overrides their shell prompt, markers break
- Interactive commands (vim, htop) show as "running" until exit

## Exit Code Fidelity
- Relies on shell prompt marker including exit code
- PowerShell uses `$LASTEXITCODE`, bash/zsh use `$?`
- Some shells may not report exit codes accurately

## PTY Session Restore
- Sessions are ephemeral per app launch
- Shell processes don't survive app restart
- Session metadata restored, but PTY must be respawned

## Planner
- Calls a local Ollama model first. The prompt includes the working directory, recent commands, known preferences, and known workflows.
- If Ollama is not running, the planner falls back to a mock. The mock recognizes a few intents and otherwise echoes the intent. The plan panel says when the source is the mock.
- Context is the current session, not a long transcript.

## Semantic Review
- Edit-and-run works, but original plan metadata not fully preserved on reopen
- Reopened plans are synthetic reconstructions from history
- No plan diffing or version tracking

## UX
- No tab reordering or renaming
- No workflow editing (only save and run)
- No memory editing (only delete + re-accept)
- Keyboard shortcuts are Ctrl-based (no customization)
- Terminal theme hardcoded (not connected to settings theme)

## Platform
- Windows: pwsh 7 preferred, fallback to powershell.exe
- macOS/Linux: reads SHELL env, fallback to /bin/bash
- No ARM-specific testing yet
