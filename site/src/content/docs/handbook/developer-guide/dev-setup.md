---
title: Development Setup
description: Prerequisites, install steps, and common commands for developing CommandUI.
sidebar:
  order: 21
---

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Node.js | 22+ | Frontend build |
| pnpm | 10+ | Package manager |
| Rust | stable (1.75+) | Tauri backend |
| Tauri CLI | 2.x | Desktop app build |

On Windows, you also need the [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and WebView2 runtime (included in Windows 11).

## Install and run

```bash
git clone https://github.com/mcp-tool-shop-org/commandui.git
cd commandui
pnpm install
```

### Browser preview (frontend only)

```bash
pnpm dev
```

Opens at `http://localhost:5176`. Uses the mock bridge — all backend operations are simulated. No Rust compilation needed. Good for UI development.

### Full Tauri app

```bash
cd apps/desktop
pnpm tauri:dev
```

Compiles the Rust backend and launches the desktop app with hot-reload. First build takes several minutes (Rust compilation). Subsequent builds are fast.

## Common commands

| Command | Scope | What it does |
|---------|-------|--------------|
| `pnpm dev` | root | Vite dev server (browser preview) |
| `pnpm typecheck` | root | TypeScript check across all packages |
| `pnpm test` | root | Run all Vitest tests |
| `pnpm build` | root | Production build (TypeScript + Vite) |
| `cd apps/desktop && pnpm tauri:dev` | desktop | Full Tauri dev app |
| `cd apps/desktop && pnpm tauri:build` | desktop | Production desktop build |
| `cd apps/desktop/src-tauri && cargo test` | backend | Rust unit tests |

## Shell detection

The app detects your shell:

- **Windows:** uses `COMMANDUI_WINDOWS_SHELL` env var if set, otherwise falls back to PowerShell (`pwsh` or `powershell.exe`)
- **Unix:** reads `$SHELL` environment variable

## App data location

| OS | Path |
|----|------|
| Windows | `%APPDATA%/com.commandui.desktop/` |
| macOS | `~/Library/Application Support/com.commandui.desktop/` |
| Linux | `~/.local/share/com.commandui.desktop/` |

SQLite database is auto-created on first launch.

## Project structure

```
commandui/
  apps/desktop/
    src/                    — React frontend
      app/AppShell.tsx      — Central orchestrator
      components/           — UI components
      features/             — Terminal/planner clients
      hooks/                — Custom React hooks
      lib/                  — Utilities, mock bridge, shortcuts
      styles/globals.css    — All CSS
    src-tauri/
      src/                  — Rust backend
        commands/           — Tauri command handlers
        ollama.rs           — LLM integration
        state.rs            — Shared app state
  packages/
    domain/                 — Pure TypeScript types
    api-contract/           — Request/response contracts
    state/                  — Zustand stores
    ui/                     — Shared UI primitives (future)
```

## Ollama (optional)

For real LLM-powered planning, install [Ollama](https://ollama.ai) and pull a model:

```bash
ollama pull llama3.2
```

The backend connects to Ollama at `http://localhost:11434` by default. If Ollama is unavailable, the planner falls back to mock responses.
