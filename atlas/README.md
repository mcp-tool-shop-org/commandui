# commandui: how it works

Mapped at 2026-10-04 from commit 435695b by Atlas 1.24.0.

## What this is

16 parts, mostly TypeScript (97 files), Rust (58), CSS (3), Astro (2), JavaScript (2), PowerShell (2) and HTML (1). Work enters through 5 doors; the busiest is CI, which reaches 10 parts. It deploys a site to GitHub Pages. People install the commandui-desktop desktop app. commandui-console is a command built from apps/console (nothing ships it).

## What changed since 2026-10-04 (7976cc3)

- CI now also runs apps/desktop/src/components/PlannerStatusCard.test.tsx, apps/desktop/src/lib/planLanguage.test.ts and crates/runtime-planner/src/status.rs.
- 5 files added and 21 changed content, across 7 parts.

## What comes in

1. **CI.** On a pull request; on a push to main touching 18 paths; or by hand. Runs packaging/check-release-version.mjs, apps/console/src/app.rs, apps/console/src/event_sink.rs and 61 more; checks apps/desktop/src/, packages/api-contract/src/, packages/domain/src/ and 4 more.
2. **Release Desktop.** When a release is published; or by hand. Runs packaging/pack-msix.ps1, apps/desktop/src-tauri/build.rs, apps/desktop/src/ and 1 more; builds apps/desktop/src-tauri/src/main.rs; checks apps/desktop/src-tauri/src/lib.rs. On a release event, it also runs packaging/check-release-version.mjs.
3. **Deploy site to GitHub Pages.** On a pull request touching 2 paths; on a push to main touching 2 paths; or by hand. Runs site/astro.config.mjs and site/src/.
4. **commandui-desktop** (the desktop app people install). Runs apps/desktop/src-tauri/src/main.rs.
5. **commandui-console** (a command built from apps/console, which nothing ships). Runs apps/console/src/main.rs.

## What happens through CI

1. The workflow runs packaging/check-release-version.mjs in packaging, packages/api-contract/src/contracts.test.ts in api-contract, 7 files in console, 30 files in desktop, packages/domain/src/memoryDetectors.test.ts and packages/domain/src/workflowPersist.test.ts in domain, and 23 files in 4 more parts; it checks packages/api-contract/src/ in api-contract, apps/desktop/src/ in desktop, packages/domain/src/ in domain, packages/state/src/ in state and packages/ui/src/ in ui.
2. It writes to apps/desktop/src-tauri/gen/schemas/.

## Who reads the results

- **apps/desktop/src-tauri/gen/schemas/** has no reader in this repository.

## The other doors

**Release Desktop** runs packaging/pack-msix.ps1, apps/desktop/src-tauri/build.rs, apps/desktop/src/ and 1 more, checks apps/desktop/src-tauri/src/lib.rs, runs packaging/check-release-version.mjs on a release event, reaches api-contract, domain, runtime-core, runtime-persistence, runtime-planner and state, writes to apps/desktop/src-tauri/gen/schemas/, and builds apps/desktop/src-tauri/src/main.rs into MSI and NSIS installers and an MSIX package, and uploads them to the release, on a release event.

**Deploy site to GitHub Pages** runs site/astro.config.mjs and site/src/, and deploys the site on a push to main or by hand.

**commandui-desktop** (the desktop app people install) runs apps/desktop/src-tauri/src/main.rs and reaches runtime-core, runtime-persistence and runtime-planner.

**commandui-console** (a command built from apps/console, which nothing ships) runs apps/console/src/main.rs and reaches runtime-core and runtime-planner.

## What breaks what

- **domain** is imported by 3 parts (api-contract, desktop, state) and sits on the path of 2 doors.
- **runtime-core** is imported by 2 parts (console, desktop) and sits on the path of 4 doors.
- **runtime-planner** is imported by 2 parts (console, desktop) and sits on the path of 4 doors.
- **api-contract** is imported by 2 parts (desktop, state) and sits on the path of 2 doors.
- **runtime-persistence** is imported by 1 part (desktop) and sits on the path of 3 doors.
- **state** is imported by 1 part (desktop) and sits on the path of 2 doors.
- **desktop** is imported by no other part and sits on the path of 3 doors.
- **console** is imported by no other part and sits on the path of 2 doors.

## What tends to change together

- **apps/console/src/model.rs** and **apps/console/src/ui.rs** changed together in 9 of 11 commits, inside the console part.
- **apps/console/src/input.rs** and **apps/console/src/model.rs** changed together in 8 of 10 commits, inside the console part.
- **apps/console/src/input.rs** and **apps/console/src/ui.rs** changed together in 8 of 10 commits, inside the console part.
- **crates/runtime-core/src/services/session_service.rs** and **crates/runtime-core/src/services/terminal_service.rs** changed together in 11 of 14 commits, inside the runtime-core part.
- **crates/runtime-core/src/pty.rs** and **crates/runtime-core/src/services/session_service.rs** changed together in 12 of 16 commits, inside the runtime-core part.

Confidence is low: fewer than 25 source files reach 10 revisions in the window.

Window: 180 days; a pair counts from 3 shared commits, since 7 source files reach 10 revisions; the floor rises to 10 when 25 do.

## What no test touches

- **ui** is imported by no test.

console is tested only by the unit tests in its own files.

runtime-persistence is tested only by the unit tests in its own files.

runtime-planner is tested only by the unit tests in its own files.

## Written but never read

- **apps/desktop/src-tauri/gen/schemas/** is written by apps/desktop/src-tauri/build.rs (a build script) and read by nothing else in this repository.

## Helpers that look duplicated

No two parts export a helper that looks alike.

## Generated, never hand-edited

- **apps/desktop/src-tauri/gen/schemas/** is written by apps/desktop/src-tauri/build.rs (a build script).

## Hand-authored

People write .claude/, .github/, docs/, packaging/, the repository root, site/ and winget/; 2 writes with paths built at run time may land here.

## Where to start

apps/desktop/src-tauri/src/main.rs → apps/desktop/src-tauri/src/lib.rs → apps/desktop/src-tauri/src/commands/planner.rs → crates/runtime-planner/src/lib.rs → crates/runtime-planner/src/client.rs → crates/runtime-planner/src/prompt.rs → crates/runtime-planner/src/types.rs

Read those in order to follow one run of commandui-desktop end to end. This path follows commandui-desktop (the desktop app people install) from its entry, since CI runs only tests, scripts that import no code here and checks.

## What this map cannot see

- 1 import could not be resolved: `apps/desktop/src/app/AppShell.honesty.test.tsx` imports `./AppShell.tsx?raw`, which is not in this repository.
- 2 writes and 3 reads use paths built at run time and are not named here.
- 2 writes and 1 read go to a path their caller passes, not to this repository.
- 3 writes go to a temporary directory, not to this repository.
- Statistics confidence is low: fewer than 25 source files reach 10 revisions in the window.

Regenerate with `npx --yes @dogfood-lab/atlas map`.
