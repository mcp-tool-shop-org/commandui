---
title: Core Concepts
description: Key abstractions in CommandUI — sessions, input modes, plans, history, workflows, memory, and risk tiers.
sidebar:
  order: 3
---

## Sessions

A session is a PTY shell process. Each session has its own terminal stream, working directory, and execution state. You can have multiple sessions open as tabs. Sessions are ephemeral — the PTY is respawned on app restart, but your history and workflows persist.

**Execution states:** `booting` → `ready` → `running` → `ready` (or `interrupting` → `ready`, or `desynced`).

## Input modes

**Command mode:** your text is sent directly to the shell as a raw command. No AI involvement.

**Ask mode:** your text is treated as natural language intent. The planner generates a command plan for your review.

The mode toggle is explicit. CommandUI never guesses which mode you meant.

## Plans

A plan is a structured proposal: the AI's translation of your intent into a shell command, annotated with risk level, explanation, assumptions, and safety flags. Plans are proposals — they never execute automatically.

**Plan lifecycle:** generated → reviewed → approved (executed) or rejected.

## History

Every interaction is recorded: raw commands, semantic requests, generated plans, approvals, rejections, and execution results. History items include the original input, the generated command (if any), the actually-executed command (if different from generated), exit code, duration, and working directory.

## Workflows

A workflow is a saved command (or multi-step sequence) promoted from history. Workflows can be:

- Created from a single successful command ("Save Workflow")
- Promoted from detected patterns (the memory system suggests sequences you repeat)
- Scoped to a project directory

Workflows are reusable — run them with one click.

## Memory

Memory items are learned preferences derived from your behavior:

- **Preferred CWD:** directories you work in frequently
- **Recurring commands:** commands you run often
- **Workflow patterns:** command sequences you repeat

Memory items have confidence scores (0–1) that increase with evidence. They are visible, editable, and deletable. The planner uses accepted memory to generate better plans.

## Risk tiers

Every generated plan has a risk assessment:

- **Low:** read-only commands (ls, git status, pwd). Visible review, no extra confirmation.
- **Medium:** commands that modify state (file moves, package install, git checkout). Configurable confirmation.
- **High:** destructive commands (recursive delete, sudo, broad writes). Required confirmation.

## Planner sources

The command planner has two backends:

- **Ollama:** local LLM generates real command plans with explanations
- **Mock:** fallback stub that echoes intent. Used when Ollama is unavailable or in browser preview mode.

The plan panel shows which source generated the current plan.
