# Input Modes: Command vs Ask

CommandUI has two input paths. Both use the same composer at the bottom of the screen.

## Command mode

Press `Ctrl+1` or click the **Command** button.

Your text is sent directly to the shell as a raw command. No AI translation, no plan panel, no confirmation step. The command executes immediately in the active session's PTY.

**When to use:** you know the exact command. `git status`, `npm test`, `docker ps`. Command mode is the terminal you already know.

**What happens:**
1. You type a command and press Enter
2. The command executes in the shell
3. Output streams to the terminal
4. A history item is created with status success/failure and exit code

## Ask mode

Press `Ctrl+2` or click the **Ask** button.

Your text is treated as natural language intent. The planner translates it into a shell command and presents a plan for review.

**When to use:** you know what you want but not the exact syntax. "Find all TypeScript files modified today." "Show disk usage sorted by size." "Kill the process on port 3000."

**What happens:**
1. You describe your intent and press Enter
2. The planner generates a `CommandPlan` with: command, explanation, risk, assumptions
3. The plan panel opens on the right side
4. You review and choose: Run Plan, edit the command, Reject, or Save Workflow
5. If approved, the command executes in the shell
6. History records both the original intent and the executed command

## The explicit toggle

CommandUI never auto-detects which mode you meant. The toggle is always visible and always manual. This is intentional:

- You are never surprised by AI involvement
- Raw commands are never intercepted for "interpretation"
- The system boundary between human input and AI output is always clear

## Composer behavior

The composer textarea supports:
- **Enter** to submit
- **Escape** to clear pending state
- Text persists when switching between modes (your draft is not lost)

When a command is running, the composer shows an **Interrupt** button instead of **Run**.

## Direct terminal typing

You can also type directly into the terminal area (click or focus it). Keystrokes go straight to the PTY — this is classic terminal behavior. Direct typing is not tracked in the structured history (it bypasses the composer).
