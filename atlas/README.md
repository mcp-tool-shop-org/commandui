# commandui: how it works

Mapped at 2026-09-24 from commit cd989a3.

## What this is

15 parts, mostly TypeScript (59 files). Work enters through 5 doors; the busiest is Release Desktop, which reaches 7 parts. People run commandui-console. People install the commandui-desktop desktop app.

## What changed since the last map

This is the first map.

## What comes in

1. **Release Desktop.** When a release is published; or by hand. Runs apps/desktop/src/ and apps/desktop/vite.config.ts; checks apps/desktop/src-tauri/src/lib.rs and apps/desktop/src-tauri/src/main.rs.
2. **CI.** On a pull request touching 9 paths; on a push touching 9 paths; or by hand. Runs crates/runtime-core/src/events.rs, crates/runtime-core/src/lib.rs, crates/runtime-core/src/parity.rs and 9 more; checks crates/runtime-persistence/src/lib.rs and crates/runtime-planner/src/lib.rs.
3. **Deploy site to GitHub Pages.** On a push to main touching 2 paths; or by hand. Runs site/astro.config.mjs and site/src/.
4. **commandui-desktop** (the desktop app people install). Runs apps/desktop/src-tauri/src/main.rs.
5. **commandui-console** (a command people run). Runs apps/console/src/main.rs.

## What happens through Release Desktop

1. The workflow runs apps/desktop/src/ and apps/desktop/vite.config.ts in desktop; it checks apps/desktop/src-tauri/src/lib.rs and apps/desktop/src-tauri/src/main.rs in desktop.
2. That reaches api-contract (12 files), domain (8 files) and state (2 files).
3. That reaches runtime-core (6 files), runtime-persistence (6 files) and runtime-planner (7 files).
4. It creates a GitHub release on a release event.

## Who reads the results

Release Desktop writes nothing this map can see.

## The other doors

**CI** runs crates/runtime-core/src/events.rs, crates/runtime-core/src/lib.rs, crates/runtime-core/src/parity.rs and 9 more, and checks crates/runtime-persistence/src/lib.rs and crates/runtime-planner/src/lib.rs.

**Deploy site to GitHub Pages** runs site/astro.config.mjs and site/src/, and deploys the site.

**commandui-desktop** (the desktop app people install) runs apps/desktop/src-tauri/src/main.rs and reaches runtime-core, runtime-persistence and runtime-planner.

**commandui-console** (a command people run) runs apps/console/src/main.rs and reaches runtime-core and runtime-planner.

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

## Written but never read

No place this map can see is written, so none goes unread.

## Helpers that look duplicated

No two parts export a helper that looks alike.

## Generated, never hand-edited

Nothing in this repository writes to a tracked place this map can see.

## Hand-authored

People write .claude/, .github/, docs/, the repository root, site/ and winget/; 1 write with a path built at run time may land here.

## Where to start

apps/desktop/src-tauri/src/main.rs

Read those in order to follow one run of commandui-desktop end to end. This path follows commandui-desktop (the desktop app people install) from its entry, since CI runs only tests.

## What this map cannot see

- 1 write and 1 read use paths built at run time and are not named here.
- Statistics confidence is low: fewer than 30 qualifying commits in the window, and fewer than 20 source files reach 10 revisions.

Regenerate with `npx --yes @dogfood-lab/atlas map`.
