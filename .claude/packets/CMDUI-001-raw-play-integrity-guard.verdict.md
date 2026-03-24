# Review Verdict — CMDUI-001

## Reviewer
Critic Reviewer

## Task ID
CMDUI-001

## Verdict
accept

## Invariant Trace

### Terminal restoration
- [x] Alternate screen re-entered: `app.rs:375` — `stdout().execute(EnterAlternateScreen)`
- [x] Cursor hidden: `app.rs:378` — `crossterm::execute!(stdout(), crossterm::cursor::Hide)`
- [x] Full redraw forced: `app.rs:381` — `terminal.clear()`
- [x] PTY resized to pane dims: `app.rs:384` — `self.sync_pane_size(terminal)`
- [x] Flag cleared: `app.rs:386` — `self.in_raw_passthrough = false`

### Session switching prohibition
- [x] Gate: `model.rs:300` — `can_switch_sessions()` returns false for `InputMode::RawPlay`
- [x] Ctrl+]/[ forwarded to PTY: `input.rs:282-324` — `handle_raw_play_key` forwards all keys except `Ctrl+\` and `Ctrl+Q`
- [x] Run selector blocked: `input.rs:282-324` — Ctrl+S not intercepted during raw play, forwarded to PTY

### stdout routing discipline
- [x] Active-only stdout: `app.rs:254-259` — `if active_id.as_deref() == Some(line_event.session_id.as_str())` gates stdout write
- [x] Non-active to model only: `app.rs:260-263` — comment confirms, `apply_event` called unconditionally, stdout write is conditional
- [x] No cross-session bleed: event matching is by `session_id` string equality, not index

### Quit path
- [x] Ctrl+Q during raw play: `input.rs:296-299` — sets `should_quit = true`, returns `ExitRawPlay`, which triggers `exit_raw_play()` before quit
- [x] Session death: `app.rs:243-248` — `raw_play_tick()` checks `active_session_alive()`, auto-exits to Shell
- [x] App cleanup: `app.rs:86-89` — checks `in_raw_passthrough`, restores alternate screen before `disable_raw_mode()`

### State coherence
- [x] Idempotent enter: `app.rs:335-336` — `if self.in_raw_passthrough { return; }`
- [x] Idempotent exit: `app.rs:370-371` — `if !self.in_raw_passthrough { return; }`
- [x] Model state accurate: `app.rs:263` — `self.model.apply_event(event)` called for all events during raw play
- [x] Proposal unaffected: proposal_session_id is independent field, not touched by raw play enter/exit

### Transcript law
- [x] Captured: `app.rs:263` — all events (including TerminalLine) routed to model during raw play
- [x] Raw storage: model stores `text` field as-is, no escape sequence interpretation (model.rs:106-110)

## Routing Verification

Tested: a packet touching raw play enter/exit should route through the harden-raw-play workflow chain (Orchestrator → Repo Researcher → Backend Engineer → Test Engineer → Critic Reviewer). The workflow exists at `.claude/workflows/harden-raw-play.md` with explicit "Use When" triggers matching raw play file paths and behavior changes.

## Rejection Verification

**Hypothetical violation:** A PR that adds Ctrl+S (run selector) support during raw play to allow session switching without exiting.

This would be caught by:
1. `harden-raw-play.md` review check: "Session switching prohibition — can_switch_sessions() returns false during RawPlay"
2. `current-priorities.md` must-preserve: "Switch blocking during raw play — no exceptions"
3. `product-brief.md` anti-thesis: "Not a split-pane productivity app"
4. Existing test: `test_switching_blocked_during_raw_play` (model.rs:795) would fail

**Verdict: reject** — the violation would be caught at 4 independent levels.

## Contract Check
- Scope respected? y — verification only, no code changes
- Output shape complete? y — all invariants traced with line numbers
- Quality bar met? y — every checklist item evidenced against source
- Risks surfaced honestly? y — no gaps found
- Ready for next stage? y — setup is locked

## Conclusion

All raw play invariants hold. Routing is correct. Critic contract catches violations at multiple levels. Role OS is tuned to CommandUI.

**Setup status: fully locked.**
