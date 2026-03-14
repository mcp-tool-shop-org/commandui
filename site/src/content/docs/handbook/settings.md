---
title: Settings and Preferences
description: Configure CommandUI behavior including mode, input defaults, risk confirmation, and display options.
sidebar:
  order: 9
---

Open settings with `Ctrl+,`. Changes take effect immediately and persist across restarts.

## Available settings

### Mode
**Options:** Classic, Guided

- **Classic:** plan panel is collapsed when empty. Terminal dominates. Minimal chrome. For users who mostly use command mode and only occasionally use semantic requests.
- **Guided:** plan panel is always visible. Shows "No semantic plan yet." when idle. Better for users learning the semantic flow or who use Ask mode frequently.

### Default input mode
**Options:** Command, Ask

Sets which mode the composer starts in when you open the app or create a new session.

### Confirm medium-risk commands
**Default:** enabled

When enabled, medium-risk plans require checking an "I understand the risks" checkbox before the Run Plan button becomes active. When disabled, only high-risk plans require confirmation.

### Reduced clutter
**Default:** disabled

Hides debug-oriented markers from the terminal output:
- `[exec:...]` execution boundary markers
- `[active]` session activity markers
- Memory suggestion panel

Enable this if you want a cleaner terminal view.

### Simplified summaries
**Default:** disabled

Trims plan explanations to the first sentence. The full explanation is still available if you re-read the plan. Useful if you find the explanations verbose.

## Settings not yet exposed

The domain types include additional settings for future expansion:

- **Theme:** system / light / dark (currently dark-only)
- **Font size:** sm / md / lg
- **Density:** compact / comfortable
- **Auto-open plan panel:** toggle
- **Explanation verbosity:** brief / normal

These exist in the type system but are not wired to UI controls yet.

## Persistence

Settings are stored in SQLite via the backend. On boot, the app loads settings and applies them to the Zustand stores. Changes are written back immediately.
