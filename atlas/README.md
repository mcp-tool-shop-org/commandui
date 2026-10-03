# commandui: how it works

Mapped at 2026-10-03 from commit 4ce0e20 by Atlas 1.24.0.

## What this is

16 parts, in TypeScript (59 files), Rust (53 files), CSS (3 files), Astro (2 files), HTML (1 file), JavaScript (1 file) and PowerShell (1 file). Work enters through 5 doors; CI and Release Desktop each reach 8 parts, and CI is followed because a pull request goes through it. It deploys a site to GitHub Pages. People install the commandui-desktop desktop app. commandui-console is a command built from apps/console (nothing ships it).

## What changed since 2026-10-01 (9b77cf9)

- CI's push trigger now also names `package.json`, `packages/**` and `pnpm-lock.yaml`.
- CI now also runs apps/console/src/input.rs, apps/console/src/model.rs, apps/console/src/planner.rs and 9 more.
- CI now also checks apps/console/src/main.rs, apps/desktop/src-tauri/src/lib.rs and apps/desktop/src-tauri/src/main.rs.
- And 1 more change to a door.
- packaging/msix/ is now read by packaging/pack-msix.ps1.
- packaging/msix/Assets/SplashScreen.scale-200.png is now read by packaging/msix/AppxManifest.xml.
- packaging/msix/Assets/Square150x150Logo.scale-200.png is now read by packaging/msix/AppxManifest.xml.
- And 4 more new writers and readers of places.
- packaging is a new part, drawn from `packaging/**`.
- 39 files added and 34 changed content, across 9 parts.

## What comes in

1. **CI.** On a pull request; on a push touching 12 paths; or by hand. Runs apps/console/src/input.rs, apps/console/src/model.rs, apps/console/src/planner.rs and 21 more; checks apps/console/src/main.rs, apps/desktop/src-tauri/src/lib.rs, apps/desktop/src-tauri/src/main.rs and 2 more.
2. **Release Desktop.** When a release is published; or by hand. Runs packaging/pack-msix.ps1, apps/desktop/src-tauri/build.rs, apps/desktop/src/ and 1 more; builds apps/desktop/src-tauri/src/main.rs; checks apps/desktop/src-tauri/src/lib.rs.
3. **Deploy site to GitHub Pages.** On a push to main touching 2 paths; or by hand. Runs site/astro.config.mjs and site/src/.
4. **commandui-desktop** (the desktop app people install). Runs apps/desktop/src-tauri/src/main.rs.
5. **commandui-console** (a command built from apps/console, which nothing ships). Runs apps/console/src/main.rs.

## What happens through CI

1. The workflow runs packages/api-contract/src/contracts.test.ts in api-contract, 4 files in console, 5 files in desktop, packages/domain/src/memoryDetectors.test.ts in domain, 6 files in runtime-core, and 7 files in 3 more parts; it checks apps/console/src/main.rs in console, apps/desktop/src-tauri/src/lib.rs and apps/desktop/src-tauri/src/main.rs in desktop, crates/runtime-persistence/src/lib.rs in runtime-persistence, and crates/runtime-planner/src/lib.rs in runtime-planner.
2. It writes to apps/desktop/src-tauri/gen/schemas/.

## Who reads the results

- **apps/desktop/src-tauri/gen/schemas/** has no reader in this repository.

## The other doors

**Release Desktop** runs packaging/pack-msix.ps1, apps/desktop/src-tauri/build.rs, apps/desktop/src/ and 1 more, checks apps/desktop/src-tauri/src/lib.rs, reaches api-contract, domain, runtime-core, runtime-persistence, runtime-planner and state, writes to apps/desktop/src-tauri/gen/schemas/, and builds apps/desktop/src-tauri/src/main.rs into MSI and NSIS installers and an MSIX package, and uploads them to the release, on a release event.

**Deploy site to GitHub Pages** runs site/astro.config.mjs and site/src/, and deploys the site.

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

No two source files changed together often enough to name.

Window: 180 days; a pair counts from 3 shared commits, since the window holds fewer than 30 qualifying commits.

## What no test touches

- **ui** is imported by no test.

console is tested only by the unit tests in its own files.

runtime-core is tested only by the unit tests in its own files.

runtime-persistence is tested only by the unit tests in its own files.

runtime-planner is tested only by the unit tests in its own files.

## Written but never read

- **apps/desktop/src-tauri/gen/schemas/** is written by apps/desktop/src-tauri/build.rs (a build script) and read by nothing else in this repository.

## Helpers that look duplicated

No two parts export a helper that looks alike.

## Generated, never hand-edited

- **apps/desktop/src-tauri/gen/schemas/** is written by apps/desktop/src-tauri/build.rs (a build script).

## Hand-authored

People write .claude/, .github/, docs/, packaging/, the repository root, site/ and winget/. Nothing in this repository writes to them.

## Where to start

apps/desktop/src-tauri/src/main.rs → apps/desktop/src-tauri/src/lib.rs → apps/desktop/src-tauri/src/commands/planner.rs → crates/runtime-planner/src/lib.rs → crates/runtime-planner/src/client.rs → crates/runtime-planner/src/prompt.rs → crates/runtime-planner/src/types.rs

Read those in order to follow one run of commandui-desktop end to end. This path follows commandui-desktop (the desktop app people install) from its entry, since CI runs only tests, scripts that import no code here and checks.

## What this map cannot see

- 1 read uses a path built at run time and is not named here.
- 1 write goes to the home directory, not to this repository.
- Statistics confidence is low: fewer than 30 qualifying commits in the window, and fewer than 25 source files reach 10 revisions.

Regenerate with `npx --yes @dogfood-lab/atlas map`.
