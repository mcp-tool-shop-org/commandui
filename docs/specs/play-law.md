# CommandUI Console — Play Law

> Doctrine amendment for terminal-native play experiences.
> Console is a live play shell with optional AI assistance, not an AI shell that happens to run games.

## Core Laws

### 1. Play comes first

During live play, the user's keystrokes own the session completely. The AI layer must never feel like it sits between the player and the game.

- In Shell mode, every keystroke goes to the game with zero interception delay
- No ambient overlays, suggestions, or notifications during active play
- The game's output owns the terminal pane — Console chrome stays out of the way

### 2. Assistance is sidecar-shaped

Ask / Review / Approve is valuable for operations *around* play:

- Launching games and managing configurations
- Saving/loading, log management
- Looking up commands or game mechanics
- Automating setup (installing, building, environment)
- Requesting hints or summaries *outside* the moment of play
- Post-session analysis or transcript review

Assistance must never turn the act of playing into approval theater. The player chooses when to engage the AI layer.

### 3. Session truth matters more for games than for shells

For games, especially multi-run or multi-character sessions:

- **Focus ownership**: which game/process owns the terminal right now
- **Transcript isolation**: which output belongs to which run — no cross-session bleed
- **Input mode clarity**: is input going to the game (raw passthrough) or to Console (mediated mode)
- **Run identity**: sessions are game runs, not just shell tabs

### 4. Fullscreen support is a declared target

Terminal-based games that expect full terminal ownership (ncurses, roguelikes, TUIs) are a real product target, not an accidental maybe.

Current state: **not yet supported** — Console is line-buffered.

Required for fullscreen:
- Raw passthrough mode where Console surrenders terminal control to the child process
- Proper terminal emulator buffer (or passthrough to host terminal)
- Clean entry/exit from fullscreen mode
- Console chrome hidden during fullscreen play

This is the next major capability gap after multi-session polish.

### 5. Proposal ownership is session-bound

A proposal belongs to the session it was generated for, not to "the app."

- If you Ask in Session A, the proposal targets Session A
- If you switch to Session B before approving, the proposal stays bound to A
- Approving a proposal executes on the proposal's session, not the currently active session
- Canceling or switching clears the review state cleanly

This prevents ambiguous execution in multi-session play.

## What Console is strong for today

First-class:
- Interactive fiction (Inform, Twine CLI, custom engines)
- MUDs and text-based multiplayer
- Line-based roguelikes (Portlight, nethack in line mode)
- CLI game engines and narrative systems
- Simulation / text-run games
- Dev-run narrative playtesting
- Any game that uses stdin/stdout without requiring full terminal control

Declared target (not yet implemented):
- ncurses games (Dungeon Crawl Stone Soup, Cataclysm DDA)
- Fullscreen roguelikes (Brogue, Angband)
- TUI games that expect full terminal ownership
- Any game using raw terminal escape sequences for rendering

## What this means for the roadmap

1. **Phase 5P** (this doc) — Play Law locked
2. **Phase 5B** — Session switcher as run selector (game-aware, not just shell tabs)
3. **Phase 5C** — Targeting integrity hardening (proposal ownership session-bound, cross-session isolation under switching)
4. **Phase 6** — Raw play mode / fullscreen passthrough investigation

## Design principle

Console's value proposition for games is not "AI plays for you."

It is: **you play freely, and when you want help — launching, configuring, understanding, analyzing — the AI is right there in the same surface, without ever getting in the way of the play itself.**
