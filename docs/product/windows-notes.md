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

The Store product is an MSIX. `packaging/msix/AppxManifest.xml` holds the identity: package name `mcp-tool-shop.CommandUI`, publisher display name `mcp-tool-shop`, executable `commandui-desktop.exe`, entry point `Windows.FullTrustApplication`, x64. The package name and publisher stay as they are. The version has to be greater than `1.0.1.0`.

`packaging/pack-msix.ps1` writes an unsigned MSIX. Partner Center signs it on ingestion. The script checks the packed manifest and refuses a signature. When the identity scanner is on the machine, a hit deletes the package.

`packaging/build-store-exe.ps1` is the release build for that package. It remaps the user-profile prefix out of the executable. A release exe built without that remap records the Cargo registry path, the identity scan hits, and `pack-msix.ps1` deletes the package.

MSI and NSIS are still the direct-download installers in `tauri.conf.json`. Their WebView2 mode is the download bootstrapper. That mode is for those installers. The Store product is the MSIX.

The MSIX does not declare a WebView2 framework package. It uses the WebView2 runtime already on the machine, same as the `1.0.1.0` package.
