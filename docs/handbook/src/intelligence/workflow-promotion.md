# Workflow Promotion

Workflow promotion is the path from detected pattern to saved workflow. It is the bridge between the memory system and the workflow system.

## The promotion flow

```
Repeated command sequence detected (Detector C)
    ↓
Suggestion surfaces: "You often run: git add → git commit → git push"
    ↓
User accepts suggestion
    ↓
Workflow editor opens with steps pre-filled
    ↓
User names workflow, optionally edits steps
    ↓
Workflow saved with source: "promoted"
    ↓
Workflow appears in drawer, feeds planner context
```

## What makes a promotable pattern

The workflow pattern detector looks for:

1. **Consecutive execution:** commands that appear in sequence within the same session
2. **Repetition:** the same sequence appears 3+ times
3. **Cross-session:** the pattern appears in 2+ different sessions (not just one burst)
4. **Success:** only successful executions count

Three-step sequences are preferred over two-step. If `A → B → C` is detected, the pairs `A → B` and `B → C` are suppressed.

## Promoted vs manual workflows

| Property | Promoted | Manual |
|----------|----------|--------|
| Source | `"promoted"` | `"raw"` or `"semantic"` |
| Badge | Shows "promoted" tag in drawer | No badge |
| Steps | Pre-filled from detected pattern | User-defined |
| Creation | From accepted suggestion | From Save Workflow button |

Both types are functionally identical once saved. The source badge is informational only.

## Workflow-aware planning

Once a workflow exists, the planner knows about it:

1. `buildPlannerContext()` includes relevant workflows as `projectFacts`
2. The Ollama prompt lists them under `## Known workflows`
3. Rule 10 in the prompt: "If the user's intent closely matches a known workflow, prefer that workflow's commands over inventing new ones."
4. The mock bridge fuzzy-matches intent against workflow labels

This means promoted workflows directly improve future plan quality. The observe → detect → promote → plan loop is complete.

## Project scoping

Promoted workflows can be project-scoped:

- If the detected pattern only occurs in one directory, the workflow's `projectRoot` is set to that directory
- Project-scoped workflows only appear in planner context when you're working in that directory
- This prevents cross-project pollution (your React workflow doesn't interfere with your Python project)
