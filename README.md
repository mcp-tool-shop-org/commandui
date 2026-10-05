<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://codecov.io/gh/mcp-tool-shop-org/commandui"><img src="https://codecov.io/gh/mcp-tool-shop-org/commandui/graph/badge.svg" alt="Coverage" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

A shell for people the terminal shuts out. CommandUI explains every result in plain words, lets you ask for a command in plain words, and never runs a drafted command until you have seen it and approved it.

## Who it is for

- People who use a screen reader, or who don't use a mouse
- People with low vision, who need larger text or a high-contrast theme
- People who find the terminal hard to follow, including beginners and people with cognitive or learning disabilities
- Anyone who wants to read a command before it runs

You still get a real shell, with your own profile and more than one session. Typing a command works the way it always has.

## Install

- **Microsoft Store:** [CommandUI on the Microsoft Store](https://apps.microsoft.com/detail/9NTN1GFQJ91M). The Store currently has an earlier version. The update described here is waiting on accessibility testing before it is submitted.
- **winget:** `winget install mcp-tool-shop.CommandUI` installs v1.0.0 from [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest).

Windows 10 or 11, x64. Ask needs [Ollama](https://ollama.com) on the same computer with the `qwen2.5:14b` model. Everything else works without it.

## What it does

- **Every result in a sentence.** "Finished. 3 lines of output." or "Did not work (exit code 1). A file or folder in that command is not there." A failure offers **Ask how to fix it** and **Run again**.
- **Ask in plain words.** Describe the task, and CommandUI drafts a command, explains it, and waits. **Run Plan** is the approval, and **Reject** runs nothing. When it cannot explain a command, it says so.
- **A careful yes.** A command that deletes files or needs higher permissions waits until you type the folder name.
- **Command still runs what you type.** If a line reads like a request, CommandUI offers to Ask instead of running the sentence.
- **Workflows you can make.** Make a list of commands, edit it, run it, and delete it. A delete can be undone. History can save the commands you pick.
- **History and memory you control.** Search what ran, and read or delete what CommandUI has noticed.

## Built for the keyboard and for screen readers

- Results and errors are announced once, without moving your focus.
- **Output** (Ctrl+Shift+O) lists each command's output as plain text, one region per command, with no terminal codes.
- **F1** opens keyboard help. **Ctrl+Shift+R** jumps to the last result. **Ctrl+Shift+A** switches between Command and Ask.
- Every dialog keeps focus inside it, and Escape closes it and returns focus to where you were.
- Text size goes from 100% to 200% in Settings. The panels below the terminal can be hidden.
- Windows contrast themes and reduced-motion settings are respected.

**What has not been tested yet:** Narrator, NVDA, and Windows contrast themes have not been tested by people on this build. Those runs come before the Store update. Until then, treat the list above as what the app is built to do, not a tested claim.

## Security

CommandUI runs on your machine. It keeps history, plans, workflows, memory, and settings locally, and runs only the shell commands you approve. It sends no telemetry. Ask talks to a model on this computer. If that model is not installed, not running, or not downloaded, Ask says so and does not draft a command.

See [SECURITY.md](SECURITY.md) for the threat model and how to report a vulnerability.

## What it is not

- Not a chatbot, and not something that runs a drafted command on its own
- Not a claim that screen readers or contrast themes have been tested on this build (see above)
- Not the console. `apps/console` is a second front end in this repo and is not part of the app you install

## For developers

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

Pack the Store upload from a release build:

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` writes `release/CommandUI_<version>_x64.msix`. It keeps the package name, publisher, and executable of the existing Store product, and refuses a version that is not above the last one submitted. The file is unsigned; Partner Center signs it.

```
commandui/
  apps/desktop/                 — the desktop app you install
  apps/console/                 — Rust terminal front end on the same runtime
  crates/runtime-core/          — shell sessions and events
  crates/runtime-persistence/   — local storage
  crates/runtime-planner/       — the local model Ask uses
  packages/                     — shared types, contracts, state, UI
  packaging/msix/               — Store manifest and logos
```

More: [Handbook](https://mcp-tool-shop-org.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## Status

v1.0.2, not yet released. The Microsoft Store has an earlier version, and the public GitHub release is v1.0.0.

Built by [MCP Tool Shop](https://mcp-tool-shop.github.io/).
