# Workflows

A workflow is a saved command or multi-step sequence that you can rerun with one click. Workflows are how CommandUI turns one-time commands into reusable tools.

## Creating workflows

There are three paths to create a workflow:

### From the plan panel
After the planner generates a command, click **Save Workflow**. The workflow editor opens with the command pre-filled.

### From history
Expand a history item and click **Save Workflow**. Works for both raw and semantic items.

### From promotion
The memory system detects command sequences you repeat (e.g., `git add → git commit → git push`). When a pattern reaches sufficient confidence, it appears as a suggestion. Accepting a workflow pattern suggestion creates a multi-step workflow.

## The workflow editor

When saving a workflow, the editor lets you:

- **Name** the workflow (label)
- **Define steps** — break a composite command into individual steps, each with its own command
- **Set project scope** — optionally restrict the workflow to a specific project directory

## Running workflows

Open the workflow drawer (`Ctrl+Shift+W`) to see all saved workflows. Each shows:

- Workflow name and source badge (`promoted` if created from pattern detection)
- Steps listed with step numbers and commands
- Original intent (if created from a semantic request)
- Creation date

Click **Run** to execute. Multi-step workflows execute sequentially: each step runs, waits for completion, then the next step starts.

## Workflow runs

When a workflow executes, a `WorkflowRun` tracks the execution:

- **Status dots** in the header show real-time progress (one dot per step)
- **Per-step tracking:** each step records status, duration, exit code
- **Stop on failure:** if a step fails, remaining steps are skipped

### Inspecting a run

Click the last-run summary line in the workflow drawer to expand run details:

- Header with overall status, start time, and total duration
- Each step as a row with: step number, status dot, command, duration
- Per-step actions: **Copy** (always), **Retry** (failed steps), **History** (view linked history item)

### Cross-drawer navigation

- From a workflow step, click **History** to jump to the history drawer with that step's history item expanded
- From a history item with a **WF** badge, click it to jump to the workflow drawer with the parent run expanded

This bidirectional link means you can always trace from a workflow run to what actually happened in the shell.

## Retry from inspection

When a workflow step fails, you have two retry options:

- **Retry Failed Step** — loads the failed command into the composer so you can edit and re-execute
- **Rerun Workflow** — re-executes the entire workflow from step 1

Both close the workflow drawer and return you to the main view.

## Project-scoped workflows

Workflows can be scoped to a project directory. A project-scoped workflow only appears in the planner's context when you're working in that directory. This prevents irrelevant workflows from polluting suggestions.

## Workflow-aware planning

The planner knows about your workflows. When you submit a semantic request that matches a known workflow's name, the planner prefers that workflow's commands over generating new ones. The context footer in the plan panel shows `workflow:<name>` when a workflow influenced the plan.
