<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-26_chapters-blue" alt="Handbook" /></a>
</p>

# CommandUI

AI-native shell environment with semantic command review.

## Install

The Microsoft Store product is an MSIX. The package name is `mcp-tool-shop.CommandUI`, x64 only. Partner Center signs the upload. The file this repo packs is unsigned, so it is the upload, not a double-click installer.

Until that upload is published, the installable build is still the MSI on [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest).

```powershell
# Scoop
scoop bucket add mcp-tool-shop https://github.com/mcp-tool-shop-org/scoop-bucket
scoop install commandui

# winget
winget install mcp-tool-shop.CommandUI
```

Pack the Store upload from a release build of the desktop app:

```powershell
pnpm --filter @commandui/desktop exec tauri build --no-bundle
./packaging/pack-msix.ps1
```

`packaging/pack-msix.ps1` writes `release/CommandUI_<version>_x64.msix`. It keeps the package name, publisher, and executable already on the Store product, and it refuses a version that is not above `1.0.1.0`.

## What it does

- Real PTY shell sessions (not a wrapper, not a chatbot)
- Two input paths: direct terminal typing (freeform) + composer (structured/tracked)
- Semantic mode: describe intent → AI generates command → you review/edit/approve
- Risk-tiered confirmation: low (auto), medium (configurable), high (required)
- History with rerun, reopen-plan, and save-to-workflow actions
- Saved workflows: promote any command to a reusable workflow
- Project-scoped memory: learns preferences from repeated edits
- Multi-session tabs with per-session terminal streams
- Local-first SQLite persistence (history, plans, workflows, memory, settings)
- Classic vs Guided modes with real behavioral differences

## What it is NOT

- Not a chatbot or autonomous agent
- Not a terminal emulator replacement
- Not the console. `apps/console` is a second front end in this repo. The Store package is the desktop executable only.

## Security

See [SECURITY.md](SECURITY.md) for the threat model and vulnerability reporting.

## Workspace layout

```
commandui/
  apps/desktop/                 — Tauri v2 + React 19. This is the Store executable.
  apps/console/                 — Rust terminal front end on the same runtime. Not in the Store package.
  crates/runtime-core/          — PTY, sessions, events
  crates/runtime-persistence/   — SQLite
  crates/runtime-planner/       — Local Ollama planner, mock fallback
  packages/domain/              — Domain types
  packages/api-contract/        — Request and response contracts
  packages/state/               — Zustand stores
  packages/ui/                  — Shared UI primitives
  packaging/msix/               — Store manifest and logos
```

## Quick start

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## Docs

- [Developer Setup](docs/product/developer-setup.md)
- [Known Limitations](docs/product/known-limitations.md)
- [Smoke Test Checklist](docs/specs/smoke-test-checklist.md)
- [Release Checklist](docs/product/release-checklist.md)

## Current status

v1.0.2 — desktop app on the shared Rust runtime. The planner calls a local Ollama model and falls back to a mock when Ollama is not running. The Store upload is the unsigned x64 MSIX of `commandui-desktop.exe`. The public GitHub release is still the v1.0.0 MSI.

Built by [MCP Tool Shop](https://mcp-tool-shop.github.io/).
