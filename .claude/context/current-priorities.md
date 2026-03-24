# Current Priorities

## Active Work
- **Manual interactive validation** — run Console through the 7-item checklist (welcome banner, ^N session create, shell commands, run selector, raw play enter/exit, help overlay). This is the last gate before release tag.

## Next Up
- Tag first Console release after manual validation passes
- Decide whether to cut a public release or move into next expansion
- If dogfood reveals a worthy gap, scope it as a Phase 8 packet through Role OS

## Blocked
- Nothing blocked. Code is pushed, tests are green, verdict is accept-with-notes.

## Recently Completed
- **Phase 7 — Product Surface** (2026-03-24): naming pass, help overlay, welcome banner, contextual footer, Console README, dogfood with 3 defects found and fixed. 101 tests. Two commits pushed to main.
- **Phases 1–6** (2026-03-23): Runtime extraction, Console bring-up, planner extraction, parity law, multi-session, play law, run selector, proposal targeting, raw play passthrough + hardening. 97→101 tests.

## Banned Detours
- Do not add workflow UI, memory UI, or history reopen to Console yet — these are Desktop features that need their own doctrine review before porting
- Do not start a plugin ecosystem — the base must be validated first
- Do not add visual polish (themes, colors, ASCII art) — function over form until post-release feedback
- Do not refactor shared crates for hypothetical future shells — two shells is the current law
- Do not expand raw play to include split-screen or real-time AI overlay — passthrough is the doctrine

## Validation Law (Non-Negotiable)

This repo is a terminal-native product. All roles must validate using terminal truth, not browser truth.

**Valid verification methods:**
- `cargo check --workspace` — compiles
- `cargo test --workspace` — 101 tests including parity drift gates
- `cargo run -p commandui-console` — manual interactive terminal session
- Code-path analysis for state machine correctness

**Prohibited verification methods:**
- No browser preview tools (`preview_start`, `preview_screenshot`, `preview_snapshot`, etc.)
- No desktop-style visual verification (screenshots, pixel diffing, layout inspection)
- No visual-polish acceptance criteria — "looks right" is not a valid check
- No GUI-style workflow validation — there is no browser, no DOM, no CSS

**Raw play lifecycle is correctness:**
- Alternate screen transitions are state machine invariants, not UI details
- Terminal restoration is a correctness requirement, not polish
- Session-switching prohibition during raw play is safety, not preference

## Must Preserve

These invariants must survive any change, even otherwise good work. Any PR, packet, or role output that weakens these is a reject.

- **Raw play integrity** — alternate screen enter/exit is idempotent, terminal always restored, no Chrome during passthrough
- **Session lifecycle truth** — events route by session_id, never by active index. Session state is owned by the model, not inferred.
- **Clean quit/restore behavior** — Ctrl+Q from any mode leaves the host terminal clean. No orphaned alternate screens, no lingering raw mode, no cursor artifacts.
- **Switch blocking during raw play** — `can_switch_sessions()` returns false during RawPlay. No exceptions. Session switching requires exiting raw play first.
- **Output routing discipline** — during raw play, only the active session's PTY output writes to host stdout. Non-active session output goes to the model only. Violation is data corruption, not a display bug.
- **Terminal-first interaction model** — Console is a TUI. Every interaction is a keystroke. No mouse, no drag, no hover, no click targets. Key ownership is exhaustive and unambiguous per mode.
- **Parity drift gate** — shared crate changes must pass parity tests in both shells. Same input + context = same proposal. Same event sequence = same state transitions.
