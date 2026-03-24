# CommandUI Console — Dogfood Log

Phase 7E/7F smoke testing via code-path analysis (Test Engineer role).

## Findings

### Defects (fixed)

| # | Finding | Evidence | Fix |
|---|---------|----------|-----|
| 1 | Welcome banner is dead code — `App::run()` creates initial session on startup, so `sessions` is never empty during render | `app.rs:78` calls `create_session()` before any render loop | Removed auto-create. Welcome banner now shows on launch. User presses `^N` to start first session. |
| 2 | Status bar "READY" conflicts with exec display "idle" — reads `READY | idle` which is redundant and contradictory | `ui.rs:101` showed "READY", `ui.rs:177` showed "idle" for the same Active+ready state | State label now hidden when `SessionState::Active` — exec display ("idle", "running", "stopping") covers it. BOOT/DONE/ERROR still show. |
| 3 | `^C Stop` removed from footer hints — interrupt undiscoverable | `ui.rs:259` had no ^C after 7A pass | Footer now contextual: shows `^C Stop` when a command is running, shows `^T Ask  ^G Raw Play` when idle. |

### Code-path analysis (verified safe)

| # | Scenario | Result |
|---|----------|--------|
| 4 | Zero sessions on startup — ^N creates first | Safe: `^N` returns `CreateSession` without checking active session |
| 5 | Zero sessions — ^S opens empty switcher | Safe: switcher handles `count == 0` by closing immediately |
| 6 | Zero sessions — ^H opens help | Safe: overlay is a boolean flag, no session dependency |
| 7 | Zero sessions — typing keys | Safe: line 183-185 returns Ignored when no active session |
| 8 | Help overlay swallows all keys except Esc/^H | Correct: ^Q caught globally before `handle_shell_key` |
| 9 | Raw play enter/exit idempotency | Correct: `in_raw_passthrough` flag guards both paths |
| 10 | Session death during raw play | Correct: `raw_play_tick` checks `active_session_alive()` and auto-exits |
| 11 | Proposal targeting survives session switch | Correct: `proposal_session_id` is independent of `active_index` |

### Platform notes

| # | Note |
|---|------|
| 12 | Ctrl+H may send backspace (0x08) on some terminals instead of `KeyCode::Char('h')` with CONTROL. Windows Terminal sends Ctrl+H correctly. Potential friction on other platforms — monitor. |

## Severity key

- **defect** — broken behavior, must fix before release
- **friction** — works but confusing or awkward, should fix
- **wish** — nice-to-have, not blocking

## Summary

3 defects found and fixed. 8 code paths verified safe. 1 platform note logged.
All 101 tests pass after fixes. Console is legible and shippable.
