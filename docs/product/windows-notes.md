# Windows Notes

## Shell Preference Order
1. `COMMANDUI_WINDOWS_SHELL` environment variable (if set)
2. PowerShell 7 (`pwsh.exe`) at `%ProgramFiles%\PowerShell\7\pwsh.exe`
3. Windows PowerShell (`powershell.exe`) fallback

## Known Test Items
- Prompt marker injection works with both pwsh and powershell
- cwd parsing handles Windows backslash paths
- Terminal resize propagates correctly
- PTY chunk boundaries may split across reads
- Keyboard focus returns to terminal after drawer close

## Packaging
- MSI and NSIS targets configured in tauri.conf.json
- No code signing configured yet (required for distribution)
