# Developer Setup

## Prerequisites

- Node.js 22+
- pnpm 10+
- Rust stable toolchain
- Tauri v2 platform dependencies ([see Tauri docs](https://v2.tauri.app/start/prerequisites/))

## Install & Run

```bash
git clone https://github.com/mcp-tool-shop-org/commandui.git
cd commandui
pnpm install

# Frontend dev server
pnpm dev

# Full Tauri dev (frontend + Rust backend)
cd apps/desktop
pnpm tauri:dev

# Tests
pnpm test           # All workspace tests
cd apps/desktop/src-tauri && cargo test  # Rust tests
```

## App Data Location

- Windows: `%APPDATA%/com.commandui.desktop/`
- macOS: `~/Library/Application Support/com.commandui.desktop/`
- Linux: `~/.local/share/com.commandui.desktop/`

SQLite database (`commandui.sqlite`) is created automatically on first launch.

## Shell Defaults

- Windows: checks `COMMANDUI_WINDOWS_SHELL` env → pwsh 7 → powershell.exe
- Unix: reads `SHELL` env → /bin/bash fallback

## Project Structure

```
apps/desktop/           — React frontend + Tauri config
apps/desktop/src-tauri/ — Rust backend
packages/domain/        — Pure TypeScript types
packages/api-contract/  — Request/response contracts
packages/state/         — Zustand stores + selectors
packages/ui/            — Shared UI primitives (future)
```
