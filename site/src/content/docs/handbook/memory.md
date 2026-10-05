---
title: "Memory System"
description: "How CommandUI learns your preferences through pattern detection, suggestions, and planner context."
sidebar:
  order: 8
---

CommandUI learns your preferences by observing patterns in your command history. Memory is narrow, transparent, and under your control.

## How it works

Three pattern detectors run on your history:

### Preferred workspace
Detects folders you work in often. It fires when you run 5 or more commands in the same folder across 2 or more sessions. Ask uses this as context when it drafts a command.

### Recurring commands
Detects commands you run often. It fires when the same command (for example `npm test` or `git status`) appears 4 or more times. Ask then prefers the tools you already use.

### Workflow patterns
Detects command sequences you repeat. If you run `git add` → `git commit` → `git push` three or more times across sessions, the system suggests promoting it to a workflow. Three-step sequences are preferred over two-step when both exist.

## Suggestions

When a detector fires, a suggestion appears at the bottom of the main view (above the composer). Each suggestion shows:

- What was detected, as a sentence, for example "You often run: git add → git commit → git push"
- How sure CommandUI is, for example "CommandUI is 65% sure."
- How many of your commands it came from, for example "Seen in 6 commands you ran."

A folder in a suggestion is shortened the same way as in the header: your home folder shows as `~`, so the account name stays off the screen.

You can:
- **Accept** — creates a memory item that the planner will use
- **Dismiss** — removes the suggestion permanently

Suggestions only appear for `pending` items. Dismissed suggestions do not return.

## Memory items

Accepted suggestions become memory items. Each item has:

| Field | Description |
|-------|-------------|
| **Kind** | Shown in plain words: Preferred workspace, Frequent command, Workflow pattern, and so on |
| **Scope** | Shown as "Everywhere", or "Only in" and the project folder |
| **Key** | Display label |
| **Value** | Stored value |
| **Confidence** | 0–1 scale, increases with evidence |
| **Source** | `observed` (from detectors), `accepted` (user confirmed), `manual` |

## Viewing and managing memory

Open the memory drawer with `Ctrl+M`. The drawer lists every accepted item: what kind it is, where it applies, and its key and value. Folders show your home folder as `~`. You can delete any item, and a screen reader hears which item each Delete button removes.

## How memory feeds the planner

When you use Ask, the planner receives your memory items as context. The `buildPlannerContext` function:

1. Resolves effective memory — merges project-scoped items (for current directory) with non-shadowed global items
2. Includes up to 5 recent commands from history
3. Includes up to 5 relevant workflows (matching current directory)
4. Packages everything as `PlannerContext` for the backend

The Ollama prompt includes a `## Known context` section with your memory items, and a `## Known workflows` section with your saved workflows. This means the planner's suggestions improve as your memory grows.

## Confidence scoring

Confidence is not binary. It scales with evidence:

- **Preferred workspace:** 0.70 base, increases with the number of commands, caps at 0.95
- **Recurring commands:** 0.60 base, increases with frequency, caps at 0.90
- **Workflow patterns:** 0.65 base, increases with repetition count, caps at 0.85

Higher confidence items are weighted more heavily in planner context.

## Project scoping

Memory items can be global or project-scoped:

- **Global:** applies everywhere
- **Project-scoped:** applies only when your current folder matches the item's project folder (`projectRoot`)

When both a global and project-scoped item exist for the same key, the project-scoped item takes precedence (shadows the global).
