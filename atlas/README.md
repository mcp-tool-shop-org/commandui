# commandui: how it works

Mapped at 2026-10-05 from commit a733e62 by Atlas 1.24.0.

## What this is

16 parts, mostly TypeScript (134 files), Rust (58), CSS (3), Astro (2), JavaScript (2), PowerShell (2) and HTML (1). Work enters through 5 doors; the busiest is CI, which reaches 10 parts. It deploys a site to GitHub Pages. commandui-console is a command built from apps/console (nothing ships it). commandui-desktop is a desktop app built from apps/desktop/src-tauri (nothing ships it).

## What changed since 2026-10-05 (4929659)

- CI's push trigger now also names `codecov.yml`.
- CI no longer runs apps/desktop/src/ and apps/desktop/vite.config.ts.
- Release Desktop no longer runs apps/desktop/src-tauri/build.rs, apps/desktop/src/ and apps/desktop/vite.config.ts.
- And 2 more changes to doors.
- README.md is now read by apps/desktop/src/lib/mockBridge.test.ts.
- 24 files added, 7 removed and 32 changed content, across 10 parts.

## What comes in

1. **CI.** On a pull request; on a push to main touching 19 paths; or by hand. Runs packaging/check-release-version.mjs, apps/console/src/app.rs, apps/console/src/event_sink.rs and 38 more; checks packages/api-contract/src/, packages/domain/src/, packages/state/src/ and 1 more.
2. **Deploy site to GitHub Pages.** On a pull request touching 2 paths; on a push to main touching 2 paths; or by hand. Runs site/astro.config.mjs and site/src/.
3. **Release Desktop.** When a release is published; or by hand. Runs packaging/pack-msix.ps1. On a release event, it also runs packaging/check-release-version.mjs.
4. **commandui-desktop** (a desktop app built from apps/desktop/src-tauri, which nothing ships). Runs apps/desktop/src-tauri/src/main.rs.
5. **commandui-console** (a command built from apps/console, which nothing ships). Runs apps/console/src/main.rs.

## What happens through CI

1. The workflow runs packaging/check-release-version.mjs in packaging, packages/api-contract/src/contracts.test.ts in api-contract, 7 files in console, 7 files in desktop, packages/domain/src/memoryDetectors.test.ts and packages/domain/src/workflowPersist.test.ts in domain, and 23 files in 4 more parts; it checks packages/api-contract/src/ in api-contract, packages/domain/src/ in domain, packages/state/src/ in state and packages/ui/src/ in ui.
2. It writes to apps/desktop/src-tauri/gen/schemas/.
3. It uploads coverage to Codecov.

## Who reads the results

- **apps/desktop/src-tauri/gen/schemas/** has no reader in this repository.

## The other doors

**Deploy site to GitHub Pages** runs site/astro.config.mjs and site/src/, and deploys the site on a push to main or by hand.

**Release Desktop** runs packaging/pack-msix.ps1, runs packaging/check-release-version.mjs on a release event, and uploads dist/**/*.exe, dist/**/*.msi and dist/**/*.msix to the release, on a release event.

**commandui-desktop** (a desktop app built from apps/desktop/src-tauri, which nothing ships) runs apps/desktop/src-tauri/src/main.rs and reaches runtime-core, runtime-persistence and runtime-planner.

**commandui-console** (a command built from apps/console, which nothing ships) runs apps/console/src/main.rs and reaches runtime-core and runtime-planner.

## What breaks what

- **domain** is imported by 3 parts (api-contract, desktop, state) and sits on the path of 1 door.
- **runtime-core** is imported by 2 parts (console, desktop) and sits on the path of 3 doors.
- **runtime-planner** is imported by 2 parts (console, desktop) and sits on the path of 3 doors.
- **api-contract** is imported by 2 parts (desktop, state) and sits on the path of 1 door.
- **runtime-persistence** is imported by 1 part (desktop) and sits on the path of 2 doors.
- **state** is imported by 1 part (desktop) and sits on the path of 1 door.
- **console** is imported by no other part and sits on the path of 2 doors.
- **desktop** is imported by no other part and sits on the path of 2 doors.

## What tends to change together

- **apps/console/src/model.rs** and **apps/console/src/ui.rs** changed together in 9 of 11 commits, inside the console part.
- **crates/runtime-core/src/services/session_service.rs** and **crates/runtime-core/src/services/terminal_service.rs** changed together in 12 of 15 commits, inside the runtime-core part.
- **apps/console/src/input.rs** and **apps/console/src/model.rs** changed together in 8 of 10 commits, inside the console part.
- **apps/console/src/input.rs** and **apps/console/src/ui.rs** changed together in 8 of 10 commits, inside the console part.
- **apps/console/src/app.rs** and **apps/console/src/model.rs** changed together in 9 of 12 commits, inside the console part.

Confidence is low: fewer than 25 source files reach 10 revisions in the window.

Window: 180 days; a pair counts from 3 shared commits, since 7 source files reach 10 revisions; the floor rises to 10 when 25 do.

## What no test touches

- **ui** is imported by no test.

console is tested only by the unit tests in its own files.

runtime-persistence is tested only by the unit tests in its own files.

runtime-planner is tested only by the unit tests in its own files.

48 test files run in no workflow: apps/desktop/src/app/AppShell.honesty.test.tsx, apps/desktop/src/app/AppShell.result.test.tsx, apps/desktop/src/app/AppShell.runtime.test.tsx and 45 more.

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

Read those in order to follow one run of commandui-desktop end to end. This path follows commandui-desktop (a desktop app built from apps/desktop/src-tauri, which nothing ships) from its entry, since CI runs only tests, scripts that import no code here and checks.

## What this map cannot see

- 130 imports could not be resolved: `apps/desktop/e2e/large-text.browser.ts` imports `@playwright/test`, which is not declared; `apps/desktop/playwright.config.ts` imports `@playwright/test`, which is not declared; `apps/desktop/src/app/AppShell.honesty.test.tsx` imports `@testing-library/react`, which is not declared; and 127 more.
- 2 writes and 3 reads use paths built at run time and are not named here.
- 3 writes go to a temporary directory, not to this repository.
- 2 writes go to a path their caller passes, not to this repository.
- Statistics confidence is low: fewer than 25 source files reach 10 revisions in the window.

Regenerate with `npx --yes @dogfood-lab/atlas map`.
