# CommandUI Console — Raw Play Mode

> Phase 6A doctrine. Defines how Console surrenders terminal ownership to fullscreen games.

## Design Law

When in raw play mode, the game owns the terminal. CommandUI becomes a thin, resumable sidecar.

## Approach: Host Terminal Passthrough

Console uses **passthrough**, not terminal emulation.

In raw play mode:
- Ratatui's alternate screen is exited — the host terminal is returned to the PTY
- PTY output is written directly to stdout (the real host terminal)
- All keystrokes are forwarded directly to the PTY — no Console interception
- Console chrome disappears completely
- The game gets real terminal behavior because it IS talking to a real terminal

This is simpler, more honest, and better than building a terminal emulator buffer.

### Why not a terminal emulator buffer?

A terminal emulator (vte, alacritty-terminal) would:
- Add massive complexity
- Re-implement what the host terminal already does perfectly
- Never be as good as the real terminal for edge cases
- Create a rendering layer between the game and the player

Passthrough gives games the real thing.

## Entry and Exit

### Entry: `Ctrl+G` (Go raw)
- Only available in Shell mode when the active session is ready
- Immediately exits Ratatui's alternate screen
- Clears the host terminal
- Resizes PTY to full host terminal dimensions (no chrome overhead)
- All subsequent keystrokes go directly to the PTY

### Exit: `Ctrl+\` (escape chord)
- This key (SIGQUIT signal key) is rarely captured by terminal programs
- Immediately re-enters Ratatui's alternate screen
- Forces a full Console redraw
- Resizes PTY back to Console's pane dimensions
- Console resumes normal operation

### Why `Ctrl+\`?
- `Ctrl+C` is used by games for interrupt
- `Ctrl+Z` is used for suspend
- `Ctrl+Q` is Console's quit key
- `Ctrl+\` is the only standard control key that games almost never capture
- It maps to SIGQUIT which is already an "escape hatch" convention

## Behavior During Raw Play

### Terminal ownership
- Game gets full terminal: rows, cols, cursor control, colors, alternate screen
- Console renders nothing — no status bar, no borders, no overlays
- The experience is identical to running the game in a naked terminal

### Keystroke forwarding
- Every key goes to the PTY — no filtering, no interception
- Only `Ctrl+\` is intercepted by Console as the escape chord
- This includes `Ctrl+C`, `Ctrl+Z`, arrow keys, function keys, escape sequences

### Resize
- Host terminal resize events propagate directly to the PTY
- Full terminal dimensions are used (no chrome subtraction)
- This is the correct behavior — the game expects to own all rows and cols

### Event routing
- Runtime events still flow through the event sink
- Terminal line events for the active session are written to stdout
- Events for other sessions are still routed to their models (has_unread still works)
- State tracking continues even though Console isn't rendering

### Session targeting
- Raw play mode applies to the active session only
- Other sessions continue receiving events normally
- Switching sessions is not available during raw play (must exit first)

## What Works in Raw Play

First-class:
- ncurses games (Dungeon Crawl Stone Soup, nethack, Angband)
- Fullscreen roguelikes (Brogue, Cataclysm DDA)
- Terminal editors (vim, nano, emacs -nw)
- System monitors (htop, top, btop)
- TUI applications (lazygit, tig, midnight commander)
- Any program that expects full terminal ownership

## What the Player Experiences

1. Playing a line-based game in Console's Shell mode
2. Decide to launch a fullscreen game: type the command, it starts but renders badly in the line buffer
3. Press `Ctrl+G` — Console disappears, game takes over the full terminal
4. Play the game natively — no Console chrome, no interference
5. Press `Ctrl+\` — Console returns, game output continues in the line buffer
6. Use Ask mode to get help, check something, then `Ctrl+G` back to raw play

That cycle — Console → raw play → Console — is the core UX.

## Lifecycle Guarantees (Phase 6B)

### Session death during raw play
If the active session closes or enters an error state while in raw play mode, Console auto-exits raw play and returns to Shell mode. The terminal is restored cleanly.

### Quit during raw play
`Ctrl+Q` during raw play first exits passthrough (restoring the alternate screen), then quits Console. The terminal is never left in a corrupted state.

### Idempotent enter/exit
`enter_raw_play()` and `exit_raw_play()` track passthrough state and are safe to call multiple times. Double-entry or double-exit is a no-op.

### Session switching blocked
Session switching (Ctrl+]/[, Ctrl+S, switcher overlay) is not available during raw play. Must exit raw play first with `Ctrl+\`.

### Background sessions
While one session is in raw play, other sessions continue receiving events normally. `has_unread` markers are still set for non-active sessions. Only the active session's output goes to the host terminal.

## Transcript Law

### What is captured
During raw play, PTY output is still delivered as `RuntimeEvent::TerminalLine` events through the event sink. These events are stored in the session model's `terminal_lines` buffer.

### What that means
- **Transcript exists** — raw play output is captured as strings
- **Transcript is raw** — terminal escape sequences (cursor movement, colors, clearing) are stored as-is
- **Transcript is not formatted** — replaying it won't reproduce the visual output
- **Transcript is searchable** — text content can be grepped/searched
- **Transcript is continuous** — no gap between Console mode and raw play mode output

### The honest claim
Transcript during raw play is **captured but degraded**. It preserves the text stream but not the visual rendering. This is a fundamental limitation of passthrough mode — the host terminal interprets the escape sequences, not Console.

## Limitations

- While in raw play mode, Console cannot render any UI
- No split-screen between Console chrome and the game
- No real-time AI assistance overlay during raw play (by design — Play Law #1)
- Session switching requires exiting raw play first
- Transcript is captured but visually degraded (escape sequences stored as raw text)
- Console has no way to detect child process exit — the session stays alive until the shell prompt returns
