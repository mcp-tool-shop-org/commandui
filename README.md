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

A shell that explains every result in plain words, lets you ask for a command in plain words, and never runs a drafted command until you have seen it and approved it.

The English page is the current description. The other languages are from the previous text and will be updated before the next release tag.

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
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`packaging/pack-msix.ps1` writes `release/CommandUI_<version>_x64.msix`. It keeps the package name, publisher, and executable already on the Store product, and it refuses a version that is not above `1.0.1.0`.

## What it does

- A real shell, with your own profile, and more than one session
- Command runs what you type. If the line looks like a request, CommandUI offers to Ask instead of running the sentence
- Ask drafts a command, explains it, and waits. Run Plan is the approval. Reject runs nothing
- A result sentence says whether the command worked. A failure offers Ask how to fix it
- Text from 100% to 200%, in Settings
- Workflows you can make, edit, run, and delete. A delete can be undone
- History you can search, and memory you can delete
- Classic hides the plan until there is one. Guided keeps the plan column open

## What it is NOT

- Not a chatbot, and not something that runs a drafted command on its own
- Not a claim that a screen reader, Narrator, or a high-contrast theme has already been tested on this build. Those checks are still open
- Not the console. `apps/console` is a second front end in this repo. The Store package is the desktop executable only.

## Security

CommandUI runs on your machine. It keeps history, plans, workflows, memory, and settings locally, and it runs the shell commands you approve. It does not send telemetry. Ask talks to a model on this computer. If that model is not installed, not running, or not downloaded, Ask says so and does not draft a command. A command that deletes files, or that needs higher permissions, waits until you type the folder name.

See [SECURITY.md](SECURITY.md) for the threat model and how to report a vulnerability.

## Workspace layout

```
commandui/
  apps/desktop/                 — the desktop app. This is the Store executable.
  apps/console/                 — Rust terminal front end on the same runtime. Not in the Store package.
  crates/runtime-core/          — shell sessions and events
  crates/runtime-persistence/   — local storage
  crates/runtime-planner/       — the local model Ask uses
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

v1.0.2 — the desktop app. Ask uses a model on this computer and says when that model is not ready. The Store upload is the unsigned x64 MSIX of `commandui-desktop.exe`, and the Store submission is still a draft. The public GitHub release is still the v1.0.0 MSI.

Built by [MCP Tool Shop](https://mcp-tool-shop.github.io/).
