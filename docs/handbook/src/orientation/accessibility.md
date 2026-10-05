# Accessibility

CommandUI is built for people the terminal shuts out: people who use a screen reader or no mouse, people with low vision, and people who find terminal output hard to follow. This page says what the app does for each, and what has not been tested yet.

## Not tested yet

Narrator, NVDA, and Windows contrast themes have **not** been tested by people on this build. Those runs come in a later update. Until then, read this page as what the app is built to do. It is not a tested claim. If something here does not work for you, please [open an issue](https://github.com/mcp-tool-shop-org/commandui/issues).

## Screen readers

- **Results are announced.** When a command finishes, its result sentence is announced once, without moving your focus. For example, "Finished. 3 lines of output." or "Did not work (exit code 1). A file or folder in that command is not there. Check the path, or ask how to fix it."
- **Errors are announced** as alerts.
- **Output as plain text.** The terminal itself has screen reader mode on, but it shows only the visible rows. **Output** (Ctrl+Shift+O) lists each command as its own named region: the command, its result sentence, then its output as plain text, with no terminal codes. Tab moves between regions. Home, End, Page Up, Page Down and the arrow keys scroll one.
- **Dialogs are dialogs.** Help, Settings, History, Workflows, Memory, Output, the workflow editor and confirmations are announced as dialogs with a name. Focus stays inside until you close them. Escape closes the top one and returns focus to where you were.
- **Plans are readable.** A drafted command comes with "What this does" and its risk in words. When CommandUI cannot explain a command, it says so: "CommandUI cannot explain this command. Read it before you run it."

## Keyboard only

Everything can be done from the keyboard. The most useful keys:

| Key | What it does |
|---|---|
| F1 | Keyboard help |
| Ctrl+J (Ctrl+Shift+J from the terminal) | Go to the command box |
| Ctrl+Shift+A | Switch between Command and Ask |
| Ctrl+Shift+R | Jump to the last result: Show output, Ask how to fix it, Run again |
| Ctrl+Shift+O | Output, as plain text |
| Ctrl+Enter | Run the plan (not while the terminal has focus) |
| Escape | Close the top dialog. It does not reject a plan. |

The terminal keeps Tab for the shell. Use Ctrl+Shift+J to get back to the command box from the terminal. The full list is on [Keyboard Shortcuts](../product-behavior/keyboard-shortcuts.md).

## Low vision

- **Text size** goes from 100% to 200% in Settings. The window keeps everything on screen at 200%. The plan moves below the terminal when the window is narrow.
- **Hide activity** and **Hide memory suggestions** give their space to the terminal. **Show** brings them back.
- **Windows contrast themes** are followed: borders, focus rings and states stay visible in a contrast theme.
- **Status is in words, not only colour.** Results, workflow steps and risks all have text.

## Motion

With **reduce motion** on in Windows, the pulsing indicators and the blinking cursor stop.

## Plain words

Results, errors and plans use everyday words. A failure says what happened, the likely cause, and what to do next. If you meet a message that uses a word you do not know, that is a bug. Please report it.
