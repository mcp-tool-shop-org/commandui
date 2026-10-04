# Handoff: TypeScript line coverage to 90%

Written 2026-10-04 on `ci/codecov` (base 04281cb). The goal is to make the
Codecov `typescript` flag a blocking 90% gate, the way `rust` already is.

## Where it stands

Measured locally with `pnpm test:coverage` (v8, `src/**` only):

| Area | Lines covered | Lines | % | Short of 90% by |
|---|---|---|---|---|
| Rust crates (for reference) | 12,629 | 13,402 | 94.2 | (passes) |
| `apps/desktop` | 815 | 4,540 | **18.0** | 3,271 lines |
| `packages/state` | 144 | 182 | 79.1 | 20 lines |
| `packages/api-contract` | 0 | 11 | 0.0 | 10 lines |
| `packages/domain` | 301 | 309 | 97.4 | (passes) |
| `packages/ui` | 0 | 0 | n/a | (no source) |

Uncovered lines in `apps/desktop`, largest first:

| Missed | File | Covered |
|---|---|---|
| 2,155 | `src/app/AppShell.tsx` | 0% |
| 382 | `src/lib/mockBridge.ts` | 0% |
| 188 | `src/components/WorkflowDrawer.tsx` | 0% |
| 128 | `src/components/WorkflowEditor.tsx` | 0% |
| 124 | `src/components/HistoryDrawer.tsx` | 47% |
| 75 | `src/components/ErrorBoundary.tsx` | 0% |
| 69 | `src/components/SettingsDrawer.tsx` | 0% |
| 59 | `src/components/TerminalPane.tsx` | 68% |
| 55 | `src/components/PlanPanel.tsx` | 74% |
| 49 | `src/lib/displaySafe.ts` | 47% |
| 47 / 44 / 39 / 39 / 39 / 39 / 36 / 33 | `MemorySuggestions`, `terminalEvents`, `MemoryDrawer`, `SessionTabs`, `terminalClient`, `tauriInvoke`, `buildPlannerContext`, `memoryClient` | 0% |
| ≤ 26 each | `WorkflowRunBanner`, `persistenceClient` (49%), `useShortcuts`, `main.tsx`, `InputComposer` (86%), `useFocusZone`, `CommandPalette` (91%), `plannerClient`, `shortcuts` (99%) | |

AppShell alone is 47% of the desktop UI's lines. No plan reaches 90% without
testing it.

## The lever: the app already runs against a fake backend

`src/lib/tauriInvoke.ts` calls `mockInvoke` from `src/lib/mockBridge.ts`
whenever `window.__TAURI_INTERNALS__` is absent, and under Vitest's jsdom it
is always absent. `onMockEvent` carries the mock's events. So
`render(<AppShell />)` boots the whole app against the mock bridge with no new
mocking layer. The same tests cover `mockBridge`, `tauriInvoke`, the
`features/*Client` modules and `terminalEvents`.

## Plan (in order; measure after each step)

1. **Move AppShell's pure helpers out and unit-test them.** Lines 100–205 hold
   module-level functions and constants: terminal replay, alt-screen and
   private-mode tracking, `simplifyText`, `detectOS`, exec-state helpers. Move
   them to `src/app/terminalReplay.ts` (and similar) and test them directly.
   This is a move, not a rewrite. Run the existing tests before and after.
2. **AppShell integration tests on the mock bridge.** Add
   `src/app/AppShell.test.tsx`. Cover boot and the boot-stall message, session
   tabs (create, switch, close), running a command and its exec states, the
   busy and exited-session messages, the plan panel's approve/reject flow,
   opening each drawer (history, memory, workflow, settings), the workflow run
   banner, and the shortcuts. Use fake timers for `BOOT_STALL_MS`,
   `FOREGROUND_STUCK_MS` and `SESSION_WATCH_MS`. xterm needs a
   canvas/ResizeObserver stub in `src/test/setup.ts` if it throws under jsdom.
3. **Component tests for the 0% drawers and editors:** WorkflowDrawer,
   WorkflowEditor, SettingsDrawer, MemoryDrawer, MemorySuggestions,
   SessionTabs, WorkflowRunBanner, ErrorBoundary (throw from a child).
   Finish HistoryDrawer, TerminalPane and PlanPanel.
4. **Libraries:** `displaySafe`, `buildPlannerContext`, `persistenceClient`,
   `useShortcuts` / `useFocusZone` (via `renderHook`).
5. **Packages:** `packages/state/src/index.ts` (38 missed lines) and
   `packages/api-contract/src/index.ts` (11 runtime lines).
6. **If AppShell still falls short,** extract the hardest-to-reach state
   machines (session watch, replay) into hooks with their own tests. Extract
   only what a test needs.

## Rules

- **Tests assert behaviour:** what the user sees, or what was sent to the
  bridge. No snapshot-only tests and no tests that only render. Coverage
  without assertions does not count.
- **Do not exclude files to reach the number.** The only exclusion to propose
  is `src/main.tsx`, the entry point. Every other exclusion needs Mike's OK,
  recorded in `codecov.yml` with its reason.
- **Do not lower the target**, and do not mark the Rust flag informational.
- Keep `pnpm typecheck`, `pnpm test` and the CI run time sane. The
  check-and-test job is about 3 minutes warm today; note the new time in the PR.

## Done when

- `pnpm test:coverage` puts every TypeScript package at 90% or more of lines
  (desktop computed from `apps/desktop/coverage/lcov.info`).
- The Codecov `typescript` flag shows 90% or more on `main`.
- `codecov.yml`: `typescript.informational` is flipped to `false` in the same
  PR. The comment in that file is updated with the new measurement.
- `atlas map` is regenerated if the source layout changed (`atlas check`
  goes red otherwise).

## Effort

About 3,300 lines to cover in the desktop UI. Steps 1–2 should cover most of
AppShell, about 2,000 lines, and the mock bridge and clients with it. Steps
3–5 close the rest. A dogfood-swarm feature pass or two focused sessions is
the right size. Split by file ownership: AppShell tests, components, libs and
packages.
