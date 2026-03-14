---
title: Adding a New Feature
description: Step-by-step guide for adding a feature to CommandUI across domain, API, state, backend, and UI layers.
sidebar:
  order: 24
---

This chapter walks through the typical path for adding a feature to CommandUI.

## Step 1: Domain types

Start in `packages/domain/`. Define any new types your feature needs. Domain types are pure TypeScript — no runtime dependencies, no React, no Zustand.

```
packages/domain/src/
  index.ts      — re-exports everything
  history.ts    — HistoryItem, etc.
  workflow.ts   — Workflow, WorkflowRun, etc.
  memory.ts     — MemoryItem, MemorySuggestion, etc.
  settings.ts   — SettingsSnapshot, etc.
```

If your feature introduces a new entity (e.g., a Snippet type), create a new file and export from `index.ts`.

## Step 2: API contract

If your feature needs backend communication, define the request/response types in `packages/api-contract/`:

```typescript
// In packages/api-contract/src/
export type MyFeatureRequest = { ... };
export type MyFeatureResponse = { ... };
```

## Step 3: State store

If your feature needs client-side state, add a store in `packages/state/src/index.ts`:

```typescript
export const useMyFeatureStore = create<MyFeatureState>()((set) => ({
  items: [],
  addItem: (item) => set((s) => ({ items: [...s.items, item] })),
  // ...
}));
```

Follow the existing pattern: state + setters, no side effects in stores.

## Step 4: Backend command (Tauri)

If your feature needs persistence or system access, add a Tauri command:

1. Create a handler in `apps/desktop/src-tauri/src/commands/`
2. Register it in `main.rs`
3. Add a client function in `apps/desktop/src/features/`

## Step 5: Mock bridge handler

Add a mock handler in `apps/desktop/src/lib/mockBridge.ts` so browser preview works:

```typescript
my_feature_action(args) {
  // Simulate the backend response
  return { ok: true, items: mockItems };
},
```

## Step 6: UI component

Create your component in `apps/desktop/src/components/`. Follow existing patterns:

- Props type at the top
- Functional component with hooks
- Presentational — receives data and callbacks, doesn't invoke backends directly

## Step 7: Wire in AppShell

AppShell is the coordinator. You'll need to:

1. Import your store and read state
2. Add handler functions for user actions
3. Pass data and callbacks as props to your component
4. Add any needed event subscriptions

## Step 8: CSS

Add styles to `apps/desktop/src/styles/globals.css`. Use existing CSS variables for colors, spacing, and radii.

## Step 9: Keyboard shortcut (optional)

If your feature needs a shortcut, add it to the shortcut definitions in AppShell and the `shortcuts.ts` module.

## Checklist

- [ ] Domain types defined
- [ ] API contract types defined (if needed)
- [ ] State store created (if needed)
- [ ] Backend command implemented (if needed)
- [ ] Mock bridge handler added
- [ ] Component created
- [ ] Wired into AppShell
- [ ] CSS added
- [ ] `pnpm typecheck` passes
- [ ] Browser preview works
- [ ] Tests added for new logic
