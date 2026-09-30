# commandui: how it works

Mapped at 2026-09-30 from commit 842976b by Atlas 1.24.0.

## What this is

15 parts, in TypeScript (59 files), Rust (53 files), CSS (3 files), Astro (2 files), HTML (1 file) and JavaScript (1 file). Work enters through 5 doors; the busiest is Release Desktop, which reaches 7 parts. It deploys a site to GitHub Pages. People install the commandui-desktop desktop app. commandui-console is a command built from apps/console (nothing ships it).

## What changed since 2026-09-24 (cd989a3)

- Release Desktop now also runs apps/desktop/src-tauri/build.rs.
- apps/desktop/src-tauri/gen/schemas/ is now written by apps/desktop/src-tauri/build.rs.
- desktop was authored and is now mixed.
- 1 file changed content, across 1 part.

## What comes in

1. **Release Desktop.** When a release is published; or by hand. Runs apps/desktop/src-tauri/build.rs, apps/desktop/src/ and apps/desktop/vite.config.ts; builds apps/desktop/src-tauri/src/main.rs; checks apps/desktop/src-tauri/src/lib.rs.
2. **CI.** On a pull request touching 9 paths; on a push touching 9 paths; or by hand. Runs crates/runtime-core/src/events.rs, crates/runtime-core/src/lib.rs, crates/runtime-core/src/parity.rs and 9 more; checks crates/runtime-persistence/src/lib.rs and crates/runtime-planner/src/lib.rs.
3. **Deploy site to GitHub Pages.** On a push to main touching 2 paths; or by hand. Runs site/astro.config.mjs and site/src/.
4. **commandui-desktop** (the desktop app people install). Runs apps/desktop/src-tauri/src/main.rs.
5. **commandui-console** (a command built from apps/console, which nothing ships). Runs apps/console/src/main.rs.

## What happens through Release Desktop

1. The workflow runs apps/desktop/src-tauri/build.rs, apps/desktop/src/ and apps/desktop/vite.config.ts in desktop; it checks apps/desktop/src-tauri/src/lib.rs in desktop.
2. That reaches api-contract (12 files), domain (8 files) and state (2 files).
3. That reaches runtime-core (6 files), runtime-persistence (6 files) and runtime-planner (7 files).
4. It writes to apps/desktop/src-tauri/gen/schemas/.
5. It builds apps/desktop/src-tauri/src/main.rs into MSI and NSIS installers and uploads them to the release on a release event.

## Who reads the results

- **apps/desktop/src-tauri/gen/schemas/** has no reader in this repository.

## The other doors

**CI** runs crates/runtime-core/src/events.rs, crates/runtime-core/src/lib.rs, crates/runtime-core/src/parity.rs and 9 more, and checks crates/runtime-persistence/src/lib.rs and crates/runtime-planner/src/lib.rs.

**Deploy site to GitHub Pages** runs site/astro.config.mjs and site/src/, and deploys the site.

**commandui-desktop** (the desktop app people install) runs apps/desktop/src-tauri/src/main.rs and reaches runtime-core, runtime-persistence and runtime-planner.

**commandui-console** (a command built from apps/console, which nothing ships) runs apps/console/src/main.rs and reaches runtime-core and runtime-planner.

## What breaks what

- **domain** is imported by 3 parts (api-contract, desktop, state) and sits on the path of 1 door.
- **runtime-core** is imported by 2 parts (console, desktop) and sits on the path of 4 doors.
- **runtime-planner** is imported by 2 parts (console, desktop) and sits on the path of 4 doors.
- **api-contract** is imported by 2 parts (desktop, state) and sits on the path of 1 door.
- **runtime-persistence** is imported by 1 part (desktop) and sits on the path of 3 doors.
- **state** is imported by 1 part (desktop) and sits on the path of 1 door.
- **desktop** is imported by no other part and sits on the path of 2 doors.

## What tends to change together

No two source files changed together often enough to name.

Window: 180 days; a pair counts from 3 shared commits, since the window holds fewer than 30 qualifying commits.

## What no test touches

- **ui** is imported by no test.

console is tested only by the unit tests in its own files.

runtime-core is tested only by the unit tests in its own files.

runtime-persistence is tested only by the unit tests in its own files.

runtime-planner is tested only by the unit tests in its own files.

3 test files run in no workflow: packages/api-contract/src/contracts.test.ts, packages/domain/src/memoryDetectors.test.ts and packages/state/src/index.test.ts.

## Written but never read

- **apps/desktop/src-tauri/gen/schemas/** is written by apps/desktop/src-tauri/build.rs (a build script) and read by nothing else in this repository.

## Helpers that look duplicated

No two parts export a helper that looks alike.

## Generated, never hand-edited

- **apps/desktop/src-tauri/gen/schemas/** is written by apps/desktop/src-tauri/build.rs (a build script).

## Hand-authored

People write .claude/, .github/, docs/, the repository root, site/ and winget/. Nothing in this repository writes to them.

## Where to start

apps/desktop/src-tauri/src/main.rs → apps/desktop/src-tauri/src/lib.rs → apps/desktop/src-tauri/src/commands/planner.rs → crates/runtime-planner/src/lib.rs → crates/runtime-planner/src/client.rs → crates/runtime-planner/src/prompt.rs → crates/runtime-planner/src/types.rs

Read those in order to follow one run of commandui-desktop end to end. This path follows commandui-desktop (the desktop app people install) from its entry, since CI runs only tests and checks.

## What this map cannot see

- 1 read uses a path built at run time and is not named here.
- 1 write goes to the home directory, not to this repository.
- Statistics confidence is low: fewer than 30 qualifying commits in the window, and fewer than 25 source files reach 10 revisions.

Regenerate with `npx --yes @dogfood-lab/atlas map`.
