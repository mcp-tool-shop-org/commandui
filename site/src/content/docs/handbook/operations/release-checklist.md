---
title: Release Checklist
description: Pre-release verification steps and smoke tests for shipping a CommandUI release.
sidebar:
  order: 26
---

## Pre-release verification

### Code quality
- [ ] `pnpm typecheck` passes (all packages)
- [ ] `pnpm test` passes (all frontend tests)
- [ ] `cargo test` passes (Rust backend, in `apps/desktop/src-tauri`)
- [ ] No console errors in browser preview

### Version consistency
- [ ] All `package.json` files show matching version
- [ ] `tauri.conf.json` version matches
- [ ] `Cargo.toml` version matches
- [ ] `APP_VERSION` constant in AppShell matches

### App sanity
- [ ] App boots without errors
- [ ] Shell session created on launch
- [ ] Raw command mode executes commands
- [ ] Semantic mode generates and reviews plans
- [ ] History persists across restart
- [ ] Settings persist across restart
- [ ] Workflows persist across restart

### Smoke test

#### Boot
- [ ] App launches clean
- [ ] Session created automatically
- [ ] Terminal shows shell output
- [ ] Header shows session label + cwd + version

#### Raw shell
- [ ] Command mode: type command → executes in shell
- [ ] Output appears in terminal
- [ ] History item created with success/failure status

#### Semantic flow
- [ ] Ask mode: submit intent → plan appears
- [ ] Edit command in plan panel
- [ ] Approve → executes edited command
- [ ] Reject → history marked rejected
- [ ] Save Workflow → workflow stored

#### Memory
- [ ] Edit a semantic command → suggestion appears after threshold
- [ ] Accept suggestion → memory item created
- [ ] Dismiss suggestion → removed
- [ ] Memory drawer shows items, delete works

#### Accessibility
- [ ] All keyboard shortcuts functional
- [ ] Ctrl+H/Ctrl+Shift+W/Ctrl+M/Ctrl+, open drawers
- [ ] Escape closes all drawers
- [ ] Classic and Guided modes behave differently

### Platform builds
- [ ] Windows: MSI/NSIS builds successfully
- [ ] macOS: DMG/app builds successfully
- [ ] Linux: deb builds successfully

### Documentation
- [ ] README current
- [ ] Known limitations updated
- [ ] Developer setup verified with clean install
- [ ] Handbook chapters accurate

## Post-release

- [ ] Git tag created (`v1.0.0`)
- [ ] Release notes drafted
- [ ] Build artifacts uploaded
