# First Ten Minutes

This chapter walks through what happens when you open CommandUI for the first time.

## Launch

The app opens with a single shell session. The header shows:

```
CommandUI v1.0.0 — ~/projects
```

The terminal area occupies the left side. The plan panel on the right shows "No semantic plan yet." The composer sits at the bottom with two mode buttons: **Command** and **Ask**.

## Try a raw command

1. Click the composer (or press `Ctrl+J`)
2. Make sure **Command** mode is selected (`Ctrl+1`)
3. Type `ls` and press Enter

The command executes in your real shell. Output appears in the terminal. A history item is created with status `success`.

## Try a semantic request

1. Switch to **Ask** mode (`Ctrl+2`)
2. Type "show me changed files" and press Enter

The planner generates a command plan. The plan panel shows:

- **Intent:** your original words
- **Command:** the generated shell command (editable)
- **Risk:** low / medium / high
- **Explanation:** why this command was chosen

You can now:

- **Run Plan** — execute the command as shown
- **Edit** the command field, then Run Plan — execute your modified version
- **Reject** — dismiss the plan, nothing executes
- **Save Workflow** — store this command for future reuse

## Explore history

Press `Ctrl+H` to open the history drawer. Every command you ran or rejected is listed with:

- Input text and source type (raw vs semantic)
- Status (success, failure, rejected, planned)
- Duration and timestamp
- Rerun, Copy, View Plan, and Save Workflow actions

Click any item to expand its details.

## Open settings

Press `Ctrl+,` to open settings. Key options:

- **Mode:** Classic (minimal chrome) vs Guided (plan panel always visible)
- **Default input:** Command or Ask
- **Confirm medium-risk:** whether medium-risk commands need explicit approval

## What to notice

After ten minutes you should have seen:

1. A real shell that executes real commands
2. An AI translation step that you fully control
3. History that accurately reflects what happened
4. The ability to edit any generated command before execution
5. Risk indication on every generated plan
