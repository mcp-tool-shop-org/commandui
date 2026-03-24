# Task Packet

## Task ID
2026-03-24-console-dogfood

## Title
Console Phase 7F — Smoke Dogfood and Hardening

## Requested Outcome
Run CommandUI Console through real terminal scenarios, log friction/defects with evidence, and fix anything that blocks legibility or shippability. Console should feel legible and shippable after this pass — not just powerful.

## User Intent
Phase 7A-7D gave Console product language, discoverability, and documentation. This packet verifies those claims against reality by running real programs through Console and logging what actually happens vs what should happen.

## Scope
- Build and launch Console (`cargo run -p commandui-console`)
- Test line-oriented programs (shell commands, REPLs)
- Test Raw Play with fullscreen programs (vim, htop)
- Test multi-session switching
- Test onboarding surfaces (welcome banner, help overlay, key hints)
- Log findings in docs/dogfood-log.md with severity (defect/friction/wish)
- Fix defects and high-friction items found

## Non-Goals
- No new features beyond what Phase 7 defined
- No plugin ecosystem, workflow UI, memory UI
- No visual polish beyond fixing discovered confusion
- No scope expansion based on wishes — log them and move on

## Inputs
- `apps/console/src/` — all Console source files (verified live)
- `apps/console/README.md` — controls reference (verified live)
- `docs/dogfood-log.md` — friction log template (verified live)
- `docs/specs/play-law.md` — play doctrine (verified live)
- `docs/specs/raw-play-mode.md` — raw play contract (verified live)

**Verification requirement:** All files verified as live in current workspace.

## Constraints
- Windows 11 terminal (Windows Terminal / cmd)
- Console is a TUI app — must be tested interactively
- Fixes must not break the 101-test suite
- No doctrine changes — only terminology/UI fixes

## Deliverable Type
Test | Code

## Assigned Role
Test Engineer → Critic Reviewer

## Upstream Dependencies
- Phase 7A-7D code complete (verified: committed as a956ca9)
- Console builds clean (verified: `cargo build -p commandui-console` succeeds)
- All 101 tests pass (verified: `cargo test --workspace` green)

**Verification requirement:** All dependencies confirmed.

## Done Definition
1. Console launched successfully and session created
2. At least 3 line-oriented scenarios tested with findings logged
3. At least 1 fullscreen Raw Play scenario tested with findings logged
4. Multi-session switching tested
5. Onboarding surfaces (welcome, help, hints) verified
6. All defect-severity findings fixed and tests still pass
7. docs/dogfood-log.md populated with real findings

## Open Questions
- Can Console actually spawn a PTY on this Windows system? (must verify)
- Does Raw Play restore correctly after vim/htop exit?

---

## Packet Type
feature

## Typical Chain
Test Engineer → Critic Reviewer
