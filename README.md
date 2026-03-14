<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
</p>

# CommandUI

AI-native shell environment with semantic command review.

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
- Not production-hardened yet

## Security

See [SECURITY.md](SECURITY.md) for the threat model and vulnerability reporting.

## Workspace layout

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
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

v1.0.0 — real shell spine with PTY sessions, semantic review loop, persistence, memory, workflows, accessibility settings, multi-session tabs, xterm.js terminal, and prompt-marker completion detection.
