# Plan Review and Approval

When you submit a request in Ask mode, the planner generates a `CommandPlan`. The plan panel opens on the right side of the screen with everything you need to make a decision.

## Plan panel anatomy

From top to bottom:

1. **Mock planner notice** (if applicable) — "Mock planner — Ollama not connected"
2. **Intent** — your original words, unmodified
3. **Command** — the generated shell command in an editable textarea
4. **Risk** — badge showing `low`, `medium`, or `high`
5. **Explanation** — why the planner chose this command
6. **Confirmation checkbox** (medium/high risk only) — "I understand the risks"
7. **Action buttons** — Run Plan, Reject, Save Workflow
8. **Context sources** — what information the planner used (cwd, workflows, memory)

## Actions

### Run Plan
Executes the command shown in the command field. If you edited it, the edited version runs.

- **Low risk:** click Run Plan directly
- **Medium risk:** requires checking the confirmation box (if "Confirm medium-risk" is enabled in settings)
- **High risk:** always requires checking the confirmation box

Shortcut: `A` (when plan panel is focused) or `Ctrl+Enter` (global)

### Reject
Dismisses the plan. Nothing executes. The plan is recorded in history with status `rejected`. Rejection is useful data — it tells the system this translation was wrong.

Shortcut: `R` (when plan panel is focused)

### Save Workflow
Saves the current command as a reusable workflow. Opens the workflow editor where you can name it and configure steps.

### Edit the command
The command field is a textarea. Click it (or press `E` when the plan panel is focused) to edit. You can modify the command freely — add flags, change paths, pipe to other commands. When you click Run Plan, your edited version executes.

History records both the original generated command and the actually-executed edited command. This edit signal is the most valuable learning data for the memory system.

## Context transparency

The footer of the plan panel shows **Context:** followed by the sources the planner used to generate this plan:

- `cwd: ~/projects` — current working directory
- `workflow:deploy` — a known workflow that influenced the plan
- `preferred_cwd:/home/dev` — a memory item that was consulted

This lets you understand *why* the planner suggested what it did.

## Reopening a plan from history

If you open the history drawer (`Ctrl+H`) and expand a semantic entry, you can click **View Plan** to reopen the plan panel with that plan's details. This lets you re-examine past plans, re-run them, or save them as workflows.
