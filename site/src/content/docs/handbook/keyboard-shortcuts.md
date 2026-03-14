---
title: Keyboard Shortcuts
description: Complete reference for all keyboard shortcuts including global, plan panel, composer, and command palette bindings.
sidebar:
  order: 10
---

CommandUI is designed for keyboard-first operation. All major actions are reachable without a pointer.

## Global shortcuts

These work from anywhere in the app:

| Shortcut | Action |
|----------|--------|
| `Ctrl+K` | Open command palette |
| `Ctrl+J` | Focus composer |
| `Ctrl+L` | Clear terminal |
| `Ctrl+T` | New session |
| `Ctrl+H` | Toggle history drawer |
| `Ctrl+Shift+W` | Toggle workflow drawer |
| `Ctrl+M` | Toggle memory drawer |
| `Ctrl+,` | Toggle settings drawer |
| `Ctrl+1` – `Ctrl+9` | Switch to session 1–9 |
| `Ctrl+W` | Close current session (Tauri only) |
| `Escape` | Close all open drawers/overlays |
| `Ctrl+Enter` | Approve and execute the current plan |

## Plan panel shortcuts

These work when the plan panel has focus:

| Shortcut | Action |
|----------|--------|
| `A` | Approve plan (same as Run Plan button) |
| `R` | Reject plan |
| `E` | Focus the command edit textarea |

## Composer shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` | Submit |
| `Ctrl+1` | Switch to Command mode |
| `Ctrl+2` | Switch to Ask mode |

## How shortcuts work

The shortcut system is zone-aware. The app tracks which zone has focus:

- **composer** — the input textarea
- **terminal** — the xterm.js terminal
- **plan** — the plan panel
- **drawer** — any open drawer
- **palette** — the command palette

### Resolution rules

1. Zone-specific matches take priority over global matches
2. Bare-key shortcuts (single letter, no modifier) are suppressed in text-input zones (terminal, composer) to avoid interfering with typing
3. Special keys (`Escape`, `Enter`, `Tab`) work in text-input zones
4. Modifier combos (`Ctrl+...`, `Shift+...`) work everywhere

### Conditional shortcuts

Some shortcuts have `when` guards — they only activate when specific conditions are true. For example, the plan approval shortcut only works when a plan is present.

## Command palette

`Ctrl+K` opens the command palette — a searchable list of all available actions. Type to filter, use arrow keys to navigate, Enter to execute. This provides discoverability for actions you might not know the shortcut for.
