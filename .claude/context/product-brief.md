# Product Brief

## Product
CommandUI is a multi-shell product family: a Tauri desktop app and a Ratatui terminal app (Console), both powered by shared runtime crates that own PTY sessions, AI planning, and persistence.

## Thesis
Terminal power users want AI assistance without losing shell ownership. CommandUI lets you play freely in a real shell, and when you want help, the AI generates reviewable commands — never intercepting your keystrokes or replacing your workflow.

## Target User
Developers and terminal power users who run interactive CLI tools, terminal games, editors, and system utilities — and want AI-assisted command generation without giving up direct shell access.

## Core Value
Real PTY shell sessions with sidecar AI: describe intent in natural language, review generated commands with risk assessment, approve or cancel. Two shells (Desktop spatial, Console terminal-native) share identical product law via extracted crates.

## Non-Goals
- Not a chatbot or conversational AI
- Not a terminal emulator replacement (wraps your shell, doesn't replace it)
- Not a remote/cloud/sync product — local-only, SQLite persistence
- Not a plugin ecosystem (yet)
- Not a visual flourish showcase — function over form

## Anti-Thesis (What CommandUI Must Never Become)

CommandUI is unusually vulnerable to false improvement. Well-intentioned changes that make it "friendlier" or "more productive" can destroy the thing that makes it work. These are hard boundaries:

- **Not a chat UI** — the AI generates commands, not conversation. No dialogue, no back-and-forth, no personality.
- **Not a desktop workflow shell** — Console is terminal-native. No draggable panels, no spatial layouts, no window management metaphors.
- **Not a visual workspace product** — no dashboards, no sidebars-of-sidebars, no "workspace overview." The terminal pane IS the workspace.
- **Not a plugin surface** — no extension points, no third-party hooks, no marketplace thinking. The product is the shell + sidecar.
- **Not a split-pane productivity app** — Console shows one session at a time. The run selector switches. No simultaneous multi-pane layouts.
- **Not a "friendlier" abstraction over terminal truth** — Console does not simplify, interpret, or soften what the shell produces. Raw output is raw. Errors are errors. The user is a terminal user.

Any change that moves Console toward these shapes is regression, not progress — even if it tests well in isolation.

## Current Stage
alpha — Console Phase 7 (product surface) complete. Accept-with-notes pending manual interactive validation before release tag. Desktop is functional but Console is the active development focus.

## Key Constraints
- Windows 11, RTX 5080 (16GB VRAM), local Ollama for AI
- Rust workspace: shared crates have zero Tauri/Ratatui deps
- Parity law: same input + context must produce same proposal in both shells
- 101 tests across workspace, 4 doctrine specs govern architecture
