# Plan Review and Approval

When you submit a request in Ask mode, the planner generates a `CommandPlan`. The plan panel opens on the right side of the screen with everything you need to make a decision.

## Plan panel anatomy

From top to bottom:

1. **Practice plan** (only a debug build or the browser preview) — "Practice plan — Ollama is not connected. This is not a real plan."
2. **Runs in** — the session the plan was made for, and its working directory. The plan always runs there, even if you have switched to another session since. If you have, the panel offers **Go to that session** and **Run in the current session instead**
3. **Intent** — your original words, unmodified
4. **Command** — the generated shell command in an editable textarea
5. **What this does** — one sentence, then the parts of the command
6. **Risk** — a sentence, such as "Low risk. Easy to undo."
7. **Folder name** (high risk only) — "Type {folder} to run this"
8. **Action buttons** — Run Plan, Reject, Save Workflow
9. **Context** — what information the planner used, when any is listed

## Actions

### Run Plan
Executes the command shown in the command field. If you edited it, the edited version runs.

- **Low risk and medium risk:** Run Plan is enough
- **High risk:** type the folder name first. There is no medium-risk checkbox

Shortcut: `A` (when plan panel is focused) or `Ctrl+Enter` (anywhere except the terminal, where it belongs to the shell)

Run is blocked when the command contains hidden characters: control characters, characters that reverse text direction, or invisible formatting characters. These can make the command on screen differ from the one the shell receives. The panel shows each one as a code such as `<U+202E>` and explains the block; remove them by editing the command. The runtime refuses such commands as well.

### Reject
Dismisses the plan. Nothing executes. The plan is recorded in history with status `rejected`. Rejection is useful data — it tells the system this translation was wrong.

Shortcut: `R` (when plan panel is focused)

### Save Workflow
Saves the current command as one workflow step. It does not open the editor. New workflow, Edit, and Save as workflow from several history rows do open the editor.

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

A plan whose session is still open runs there. If that session has been closed, or it belongs to an earlier launch, the panel says so: you can run it in the current session instead, or keep it open read-only. Re-running a command or a workflow asks for confirmation when the session's working directory differs from the one it was planned for.
