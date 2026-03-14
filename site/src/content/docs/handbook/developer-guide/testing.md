---
title: Testing
description: Test framework, test locations, and principles for testing CommandUI frontend and backend.
sidebar:
  order: 23
---

## Test framework

- **Frontend:** Vitest + React Testing Library + jsdom
- **Backend:** Rust's built-in `#[test]` with `cargo test`

## Running tests

```bash
# All frontend tests
pnpm test

# Watch mode
cd apps/desktop && pnpm vitest

# Rust tests
cd apps/desktop/src-tauri && cargo test
```

## Test locations

| Test file | What it covers |
|-----------|---------------|
| `apps/desktop/src/components/InputComposer.test.tsx` | Composer rendering, mode toggle, submit |
| `apps/desktop/src/components/PlanPanel.test.tsx` | Plan display, risk confirmation, edit-and-approve |
| `apps/desktop/src/lib/shortcuts.test.ts` | Shortcut parsing, zone matching, combo resolution |
| `packages/domain/src/memoryDetectors.test.ts` | All three detectors: thresholds, confidence, dedup |
| `packages/state/src/index.test.ts` | Zustand store operations |
| `packages/api-contract/src/contracts.test.ts` | Type contract validation |

## Rust tests

The planner module has comprehensive tests:

| Test | What it verifies |
|------|-----------------|
| `test_mock_planner_git_status` | Mock recognizes "changed files" intent |
| `test_mock_planner_destructive` | Mock flags "delete" as high risk |
| `test_mock_planner_generic` | Mock generates stub for unknown intents |
| `test_llm_response_conversion` | LLM response maps correctly to plan payload |
| `test_llm_response_safety_flags` | Destructive flag generates DESTRUCTIVE_OPERATION |

## Testing principles

### Domain logic is pure
Memory detectors, context builders, and type utilities are pure functions. They take data in, return data out, with no side effects. Test them with simple input/output assertions.

### Components are presentational
Most components receive props and callbacks. Test them with React Testing Library: render with props, assert DOM content, simulate user actions, verify callback invocations.

### State stores are isolated
Zustand stores can be tested independently. Create a store, call actions, assert state changes.

### Mock bridge is the integration test
The mock bridge provides a complete simulation of the backend. Running the app in browser preview and interacting with it exercises the full frontend stack against plausible responses.

## What to test when adding features

1. **New domain type?** Add type assertions in `contracts.test.ts`
2. **New detector?** Add detector tests in `memoryDetectors.test.ts`
3. **New component?** Add component tests with React Testing Library
4. **New shortcut?** Add shortcut test in `shortcuts.test.ts`
5. **New Tauri command?** Add Rust tests in the command module
6. **New mock handler?** Test through browser preview manually

## Smoke testing

The [Smoke Test Checklist](../operations/release-checklist.md) provides a manual verification sequence covering boot, raw shell, semantic flow, persistence, memory, and accessibility.
