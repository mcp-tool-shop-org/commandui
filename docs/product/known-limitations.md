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

## Planner Context
- v0 uses a mock planner (no real AI integration yet)
- Memory items fed to planner but not used by stub
- Limited context assembly (cwd, recent commands, memory)

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
