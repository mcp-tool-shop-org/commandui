# Keyboard Shortcuts

CommandUI is designed for keyboard-first operation. All major actions are reachable without a pointer.

## Global shortcuts

While the terminal has focus, plain `Ctrl+<letter>` belongs to the shell: `Ctrl+W` deletes a word, `Ctrl+K` kills to end of line, `Ctrl+L` clears the screen, and so on. Each app action that uses a plain `Ctrl+<letter>` therefore has a `Ctrl+Shift` form that works everywhere, the terminal included.

| Shortcut | Outside the terminal | Everywhere | Action |
|----------|----------------------|------------|--------|
| Palette | `Ctrl+K` | `Ctrl+Shift+K` | Open command palette |
| Composer | `Ctrl+J` | `Ctrl+Shift+J` | Focus composer |
| Clear | `Ctrl+L` | `Ctrl+Shift+L` | Clear terminal view |
| New session | `Ctrl+T` | `Ctrl+Shift+T` | New session |
| History | `Ctrl+H` | `Ctrl+Shift+H` | Toggle history drawer |
| Memory | `Ctrl+M` | `Ctrl+Shift+M` | Toggle memory drawer |
| Close session | `Ctrl+W` | `Ctrl+Shift+X` | Close current session (Tauri only); asks first if a command is running |

These have no shell meaning and work from anywhere, the terminal included:

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+W` | Toggle workflow drawer |
| `Ctrl+,` | Toggle settings drawer |
| `Ctrl+1` – `Ctrl+9` | Switch to session 1–9 |
| `Escape` | Close all open drawers/overlays |
| `Ctrl+Enter` | Approve and execute the current plan, on the session it was planned for |

## Plan panel shortcuts

These work when the plan panel has focus:

| Shortcut | Action |
|----------|--------|
| `A` | Approve plan (same as Run Plan button) |
| `R` | Reject plan |
| `E` | Focus the command edit textarea |

## Composer shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` | Submit |
| `Ctrl+1` | Switch to Command mode |
| `Ctrl+2` | Switch to Ask mode |

## How shortcuts work

The shortcut system is zone-aware. The app tracks which zone has focus:

- **composer** — the input textarea
- **terminal** — the xterm.js terminal
- **plan** — the plan panel
- **drawer** — any open drawer
- **palette** — the command palette

### Resolution rules

1. Zone-specific matches take priority over global matches
2. Bare-key shortcuts (single letter, no modifier) are suppressed in text-input zones (terminal, composer) to avoid interfering with typing
3. Special keys (`Escape`, `Enter`, `Tab`) work in text-input zones
4. Modifier combos (`Ctrl+...`, `Shift+...`) work everywhere, except plain `Ctrl+<letter>` while the terminal has focus, which goes to the shell

### Conditional shortcuts

Some shortcuts have `when` guards — they only activate when specific conditions are true. For example, the plan approval shortcut only works when a plan is present.

## Command palette

`Ctrl+K` opens the command palette — a searchable list of all available actions. Type to filter, use arrow keys to navigate, Enter to execute. This provides discoverability for actions you might not know the shortcut for.
