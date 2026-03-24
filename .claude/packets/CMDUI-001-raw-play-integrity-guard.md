# Task Packet

## Task ID
CMDUI-001

## Title
Raw Play Integrity Guard — Proving Packet

## Requested Outcome
Verify that all raw play invariants hold against current code, that Role OS routing picks the correct chain for raw-play-touching work, and that the Critic Reviewer can reject any solution that weakens terminal truth.

This is a proving packet: it validates that Role OS is tuned to CommandUI, not just generically initialized.

## User Intent
Role OS must protect CommandUI from the exact ways generic orchestration would damage it. Raw play is the highest-risk architectural seam. If the proving packet passes, the setup is locked. If it fails, the setup is not CommandUI-safe yet.

## Scope
1. **Invariant verification** — trace every raw play invariant listed in `workflows/harden-raw-play.md` against current source code
2. **Routing verification** — confirm `roleos route` recommends the correct chain for a raw-play-touching change
3. **Rejection verification** — confirm the Critic Reviewer contract and harden-raw-play checklist would catch a hypothetical violation (e.g., allowing session switching during raw play)

## Non-Goals
- No code changes — this is verification only
- No new tests — existing 101 tests + code-path analysis
- No doctrine amendments

## Inputs
- `apps/console/src/app.rs` — raw play lifecycle (verified live)
- `apps/console/src/input.rs` — raw play key handler (verified live)
- `apps/console/src/model.rs` — session switching gate, session alive check (verified live)
- `docs/specs/raw-play-mode.md` — Phase 6A doctrine (verified live)
- `docs/specs/play-law.md` — play-first doctrine (verified live)
- `.claude/workflows/harden-raw-play.md` — repo-local workflow (verified live)

**Verification requirement:** All files verified as live in current workspace.

## Constraints
- Verification is terminal/test/code-path only — no browser preview, no visual tools
- Must use the harden-raw-play.md workflow review checklist
- Must trace invariants to specific line numbers in source

## Deliverable Type
Review

## Assigned Role
Repo Researcher → Test Engineer → Critic Reviewer

## Upstream Dependencies
- Console builds clean (verified: `cargo build -p commandui-console` succeeds)
- All 101 tests pass (verified: `cargo test --workspace` green)
- harden-raw-play.md workflow exists (verified: created this session)

**Verification requirement:** All dependencies confirmed.

## Done Definition
1. Every invariant in harden-raw-play.md traced to source code with line numbers
2. `roleos route` for a raw-play-touching packet recommends the correct chain
3. At least one hypothetical violation described and confirmed rejectable by Critic contract
4. Verdict issued: accept (setup is locked) or reject (gaps remain)

## Open Questions
None — this is verification of existing code against existing doctrine.

---

## Packet Type
feature

## Typical Chain
Repo Researcher → Test Engineer → Critic Reviewer
