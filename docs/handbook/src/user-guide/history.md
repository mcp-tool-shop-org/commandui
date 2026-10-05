# History and Rerun

Every structured interaction — raw commands through the composer and semantic requests — is recorded in history. Open the history drawer with `Ctrl+H`.

## History item fields

Each entry records:

| Field | Description |
|-------|-------------|
| **User input** | What you typed (command or intent) |
| **Source, on screen** | Typed command, From Ask, or From Ask, practice plan |
| **Generated command** | The drafted command, when Ask made one |
| **Executed command** | What actually ran (may differ if you edited) |
| **Status** | `success`, `failure`, `rejected`, `planned`, `interrupted` |
| **Exit code** | Shell exit code (0 = success) |
| **Duration** | How long execution took |
| **Working folder** | The folder the command ran in. The row shows it the same way as the header, with `~` for the home folder. |
| **Planner source, stored** | The model, or a practice plan |
| **Workflow run ID** | Links to parent workflow run, if applicable |

## Browsing history

The history drawer provides:

- **Search** — filter by text across user input, generated command, and executed command
- **Session filter** — view current session, all sessions, or a specific session
- **Expand/collapse** — click any item to see full details

## Actions on history items

Click an item to expand it. The detail view shows all fields plus action buttons:

### Rerun
Re-executes the same command. Loads it into the composer and submits.

### Copy
Copies the command to clipboard.

### View Plan
(Semantic items only) Reopens the plan panel with the original plan details. Available when the item has a linked plan.

### Save Workflow
Saves the command as a reusable workflow. Opens the workflow editor.

## Workflow-linked items

A command that was part of a workflow run has a **View workflow** button on its row. That button opens the workflow run. The expanded row has the same action, named **View workflow run**.

## Persistence

History persists across app restarts via SQLite. The app loads the most recent 100 items on boot. Items are never automatically deleted.

## What is NOT in history

Direct terminal typing (keystrokes sent to the PTY by clicking the terminal) bypasses the composer and is not recorded in structured history. Only commands submitted through the composer are tracked.
