# Workflow: Harden Raw Play

## Use When
- Work touches raw play mode (enter, exit, passthrough loop, escape chord)
- Session ownership boundaries change
- Terminal lifecycle behavior changes (alternate screen, cursor, raw mode)
- Output routing changes (which session writes to stdout)
- Quit/cleanup behavior changes
- Any change to `app.rs:enter_raw_play`, `app.rs:exit_raw_play`, `app.rs:raw_play_tick`
- Any change to `input.rs:handle_raw_play_key`
- Any change to `model.rs:can_switch_sessions`, `model.rs:active_session_alive`

## Required Chain
1. **Orchestrator** — verify the change is scoped and does not violate raw play doctrine
2. **Repo Researcher** — read `docs/specs/raw-play-mode.md` and `docs/specs/play-law.md` to confirm the change is compatible with doctrine
3. **Backend Engineer** — implement the change, ensuring all invariants hold
4. **Test Engineer** — verify via tests and code-path analysis
5. **Critic Reviewer** — accept only if all review checks pass

## Required Review Checks

The Critic Reviewer must verify each of these against evidence (test output, code-path trace, or manual verification). A missing check is a reject.

### Terminal restoration
- [ ] After exiting raw play, alternate screen is re-entered
- [ ] Cursor is hidden (Ratatui manages it)
- [ ] Full redraw is forced (`terminal.clear()`)
- [ ] PTY is resized back to Console pane dimensions
- [ ] `in_raw_passthrough` flag is set to false

### Session switching prohibition
- [ ] `can_switch_sessions()` returns false during `InputMode::RawPlay`
- [ ] Ctrl+]/[ do not switch sessions during raw play (they're forwarded to PTY)
- [ ] Run selector cannot be opened during raw play

### stdout routing discipline
- [ ] Only the active session's TerminalLine events write to host stdout
- [ ] Non-active session output goes to model only (for unread tracking)
- [ ] No cross-session stdout bleed under any event ordering

### Quit path
- [ ] Ctrl+Q during raw play: exits raw mode first, then quits
- [ ] Session death during raw play: auto-exits to Shell mode, terminal restored
- [ ] App cleanup (`App::run` exit path): restores alternate screen if `in_raw_passthrough` is true
- [ ] Terminal is always left clean regardless of exit path

### State coherence
- [ ] `in_raw_passthrough` flag is consistent with `InputMode::RawPlay`
- [ ] Enter/exit transitions are idempotent (calling enter when already in is a no-op)
- [ ] Model state (session_state, exec_state, unread flags) remains accurate during raw play
- [ ] Proposal ownership is unaffected by raw play enter/exit

### Transcript law
- [ ] PTY output during raw play is still captured in the model as TerminalLine events
- [ ] Transcript is raw (escape sequences as-is) — no attempt to interpret or format

## Reject Criteria

Automatic reject if any of:
- Terminal is not restored after raw play exit
- Session switching is possible during raw play
- stdout contains output from a non-active session during raw play
- Quit path leaves terminal in a dirty state
- `in_raw_passthrough` flag is inconsistent with actual terminal state
- Change contradicts `docs/specs/raw-play-mode.md` without doctrine amendment

## Doctrine References
- `docs/specs/raw-play-mode.md` — Phase 6A canonical spec
- `docs/specs/play-law.md` — Play-first doctrine, sidecar model
- `docs/specs/tui-spine.md` — One Runtime, Two Adapters architecture
