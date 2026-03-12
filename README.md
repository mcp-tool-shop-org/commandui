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
- Not production-hardened (early v0)

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

Early v0 with a real shell spine. 21-piece bootstrap delivering: PTY sessions, semantic review loop, persistence, memory, workflows, accessibility settings, multi-session tabs, xterm.js terminal, prompt-marker completion detection.
