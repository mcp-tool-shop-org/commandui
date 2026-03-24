# CommandUI Parity Law

> Phase 4 doctrine. Protects the product family from silent drift.

## Must Be Identical Across Desktop and Console

These are **shared law**. If they differ between shells, the product family is broken.

### Proposal truth
- `CommandProposal` shape and field semantics
- `PlanContext` shape
- `PlanReview` shape and safety flag derivation
- Risk model: "low" / "medium" / "high" — same thresholds, same meaning
- Confidence model: 0.0–1.0, same interpretation
- Validation rules: fail-closed, same rejections for same malformed input
- Prompt template: same structure, same rules section
- Mock fallback: same keyword matching, same proposal shape

### Session/terminal truth
- `RuntimeEvent` enum and all 6 event type payloads
- Session lifecycle: Booting → Active → Closed
- Exec state transitions: booting → ready → running → ready (or interrupting → ready)
- Prompt-marker parsing and boot detection
- Execution completion inference
- CWD tracking from prompt markers
- Interrupt and resync semantics

### Persistence truth
- Schema shape
- History/settings/workflow/memory CRUD semantics
- Plan storage shape

### Execution truth
- `ExecuteRequest` shape
- Source tagging: "ask" for intent-mode, "direct" for shell-mode
- Risk-gated execution: same conditions require same confirmation

## Allowed to Differ

These are **shell-specific**. Differences are expected and correct.

### Rendering
- Layout system (Ratatui widgets vs React components)
- Color palette and theme
- Typography and density
- Status bar content and arrangement
- Review panel visual design
- Animation and transitions

### Interaction flow
- Mode transitions and keybindings
- Composer implementation (text input widget vs terminal composer)
- Review approval UX (keyboard-driven vs mouse+keyboard)
- Overlay/drawer/panel navigation
- Scrollback implementation

### Transport
- Event delivery mechanism (ChannelSink vs TauriEventSink)
- Request/response envelope shape (Tauri commands vs direct service calls)
- Async plumbing (tokio channels vs Tauri event bridge)

### Platform
- Terminal capabilities (Console: line-buffered, no fullscreen children yet)
- Window management (Desktop: native window chrome)
- System integration (Desktop: tray, notifications)

## Capability Declaration

### Console (current)
- ✅ Real shell session with PTY ownership
- ✅ Direct terminal typing (Shell mode)
- ✅ Ask → Review → Approve (Intent mode)
- ✅ Risk display and safety flags
- ✅ Scrollback with buffer cap (10K lines)
- ✅ Interrupt and resync
- ✅ Resize law (chrome-first, derived PTY geometry)
- ⚠️ Line-buffered output (no terminal emulator buffer)
- ⚠️ Fullscreen child apps not first-class (vim, htop render incorrectly)
- ⚠️ Single session only (multi-session is next expansion)
- ❌ No workflows UI
- ❌ No memory UI
- ❌ No history reopen/rerun UI

### Desktop (current)
- ✅ Real shell session with PTY ownership
- ✅ Direct terminal typing
- ✅ Ask → Review → Approve with spatial review panel
- ✅ Risk display and safety flags
- ✅ Multi-session with tabs
- ✅ Workflows editor
- ✅ Memory management
- ✅ History with rerun/reopen-plan
- ✅ Settings drawer
- ✅ Richer spatial layout

## Drift Gate

### Tests that enforce parity
- Same intent + same context → same `CommandProposal` (modulo id/timestamp)
- Same malformed LLM JSON → same validation rejection
- Same runtime event sequence → same semantic state transitions
- Validation consistency checks (destructive+low=rejected, escalation+no-approval=rejected)

### Change protocol
When shared-law behavior changes:
1. Change lives in the shared crate (runtime-core, runtime-persistence, or runtime-planner)
2. Parity fixtures are updated to match
3. Both shells must still compile and pass
4. PR description notes what shared law changed and why

Shell-specific changes do not require parity review.
