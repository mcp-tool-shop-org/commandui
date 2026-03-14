---
title: Context Enrichment
description: How CommandUI assembles memory, workflows, and session state into rich planner context.
sidebar:
  order: 20
---

Context enrichment is the process of assembling everything the planner needs to generate a good command plan. Without context, the planner is a talented stranger — it knows shell syntax but nothing about your project.

## The buildPlannerContext function

`buildPlannerContext()` is a pure function that takes raw inputs and produces a structured `PlannerContext`. It lives in `apps/desktop/src/lib/buildPlannerContext.ts`.

### Inputs

| Input | Source | Purpose |
|-------|--------|---------|
| `sessionId` | Active session | Links plan to session |
| `cwd` | Session state | Current working directory |
| `shell` | Session state | Shell type (bash, pwsh, etc.) |
| `os` | Runtime detection | Operating system |
| `memoryItems` | MemoryStore | All accepted memory items |
| `workflows` | WorkflowStore | All saved workflows |
| `lastRunByWorkflowId` | WorkflowRunStore | Last run status per workflow |
| `recentHistory` | HistoryStore (filtered) | Current session's history |

### Processing

1. **Effective memory resolution:** calls `resolveEffectiveMemory(items, cwd)` which merges project-scoped items (for the current directory) with non-shadowed global items. Project-scoped items take precedence over globals with the same key.

2. **Recent commands:** extracts the last 5 executed commands from session history. Gives the planner awareness of what you just did.

3. **Relevant workflows:** filters to workflows matching the current directory (or global workflows), takes the top 5. Each workflow becomes a `projectFact` with:
   - `kind: "workflow"`
   - `label:` workflow name
   - `value:` step commands joined with " → ", plus last-run status if available

### Output

```typescript
{
  sessionId, cwd, projectRoot, os, shell,
  recentCommands: string[],
  memoryItems: { kind, key, value, confidence }[],
  projectFacts: { kind, label, value }[]
}
```

## How the backend uses context

### Ollama prompt

The `build_planner_prompt()` function in Rust formats the context into the LLM prompt:

- **System section:** cwd, OS, shell
- **Memory section:** `## Known context` with each memory item
- **Workflow section:** `## Known workflows` with each workflow's commands and last-run status
- **Recent commands:** listed for recency awareness
- **Rules:** 10 rules including "prefer known workflow commands over inventing new ones"

### Mock planner

The mock fallback uses context for:
- Setting `cwd` on the response
- Populating `retrievedContext` in the plan review
- Workflow matching: fuzzy-matching intent against workflow labels to return the workflow's commands instead of a generic stub

### Plan review

`retrievedContext` in the `PlanReviewPayload` lists what context was assembled:
- `cwd: ~/projects`
- `projectRoot: ~/projects`
- `workflow:deploy`
- `workflow:test-suite`

This is displayed in the PlanPanel footer so you can see what influenced the plan.

## The enrichment loop

Context enrichment creates a virtuous cycle:

```
Use CommandUI → History accumulates
    ↓
Detectors find patterns → Suggestions appear
    ↓
Accept suggestions → Memory items created
    ↓
Save workflows → Workflow library grows
    ↓
Submit semantic request → Rich context assembled
    ↓
Planner generates better plan → User approves
    ↓
More history → Better detection → Richer context
```

The system gets better the more you use it, without any explicit training step.
