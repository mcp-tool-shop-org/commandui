---
title: "Memory System"
description: "How CommandUI learns your preferences through pattern detection, suggestions, and planner context."
sidebar:
  order: 8
---

CommandUI learns your preferences by observing patterns in your command history. Memory is narrow, transparent, and under your control.

## How it works

Three pattern detectors run on your history:

### Preferred CWD
Detects directories you work in frequently. Triggers when you have 5+ executions across 2+ sessions in the same directory. The planner uses this to set context when generating commands.

### Recurring commands
Detects commands you run often. Triggers when the same command family (e.g., `npm test`, `git status`) appears 4+ times. Helps the planner suggest familiar tools.

### Workflow patterns
Detects command sequences you repeat. If you run `git add` → `git commit` → `git push` three or more times across sessions, the system suggests promoting it to a workflow. Three-step sequences are preferred over two-step when both exist.

## Suggestions

When a detector fires, a suggestion appears at the bottom of the main view (above the composer). Each suggestion shows:

- What was detected (e.g., "You frequently run 'npm test'")
- Confidence score as a percentage
- Evidence (execution count)

You can:
- **Accept** — creates a memory item that the planner will use
- **Dismiss** — removes the suggestion permanently

Suggestions only appear for `pending` items. Dismissed suggestions do not return.

## Memory items

Accepted suggestions become memory items. Each item has:

| Field | Description |
|-------|-------------|
| **Kind** | Category: `preferred_cwd`, `recurring_command`, `workflow_pattern`, etc. |
| **Scope** | `global` or `project` (project-scoped items apply only in their directory) |
| **Key** | Display label |
| **Value** | Stored value |
| **Confidence** | 0–1 scale, increases with evidence |
| **Source** | `observed` (from detectors), `accepted` (user confirmed), `manual` |

## Viewing and managing memory

Open the memory drawer with `Ctrl+M`. The drawer lists all accepted memory items with their kind, scope, key, value, and project root (if applicable). You can delete any item.

## How memory feeds the planner

When you submit a semantic request, the planner receives your memory items as context. The `buildPlannerContext` function:

1. Resolves effective memory — merges project-scoped items (for current directory) with non-shadowed global items
2. Includes up to 5 recent commands from history
3. Includes up to 5 relevant workflows (matching current directory)
4. Packages everything as `PlannerContext` for the backend

The Ollama prompt includes a `## Known context` section with your memory items, and a `## Known workflows` section with your saved workflows. This means the planner's suggestions improve as your memory grows.

## Confidence scoring

Confidence is not binary. It scales with evidence:

- **Preferred CWD:** 0.70 base, increases with execution count, caps at 0.95
- **Recurring commands:** 0.60 base, increases with frequency, caps at 0.90
- **Workflow patterns:** 0.65 base, increases with repetition count, caps at 0.85

Higher confidence items are weighted more heavily in planner context.

## Project scoping

Memory items can be global or project-scoped:

- **Global:** applies everywhere
- **Project-scoped:** applies only when your current directory matches the item's `projectRoot`

When both a global and project-scoped item exist for the same key, the project-scoped item takes precedence (shadows the global).
