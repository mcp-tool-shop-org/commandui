---
title: Planner Pipeline
description: Full pipeline from natural language intent to structured command plan, covering context assembly, LLM generation, safety annotation, and user review.
sidebar:
  order: 16
---

The planner translates natural language intent into a structured command plan. This chapter traces the full pipeline from user input to plan presentation.

## Pipeline stages

```
User intent (Ask mode)
    ↓
1. Context assembly (frontend)
    ↓
2. Plan generation (backend: Ollama or mock)
    ↓
3. Safety annotation (backend)
    ↓
4. Plan presentation (frontend: PlanPanel)
    ↓
5. User decision: approve / edit / reject / save
```

## Stage 1: Context assembly

The `buildPlannerContext()` pure function assembles a `PlannerContext` from:

- **Session info:** sessionId, cwd, shell, OS
- **Effective memory:** project-scoped items merged with non-shadowed globals (via `resolveEffectiveMemory`)
- **Recent commands:** last 5 executed commands from history
- **Relevant workflows:** up to 5 workflows matching the current directory, with last-run status

This context travels with the plan request to the backend.

**File:** `apps/desktop/src/lib/buildPlannerContext.ts`

## Stage 2: Plan generation

The backend receives the request and tries Ollama first:

### Ollama path
The `build_planner_prompt()` function constructs an LLM prompt with:

- System instructions (respond as JSON, specific fields required)
- User's cwd, OS, shell
- Memory items as `## Known context`
- Workflows as `## Known workflows` (with rule: prefer known workflow commands over inventing new ones)
- Recent command history
- 10 rules for plan quality (confidence scoring, risk assessment, explanation requirements)

Ollama returns a `LlmPlanResponse` with: command, risk, explanation, assumptions, safety flags, confidence, expected output.

### When the model is not ready
A release call returns no command. It returns the status: not installed, not running, or not downloaded.

A debug build, tests, and the browser preview may still build a practice plan. The screen says "Practice plan — Ollama is not connected. This is not a real plan." The stored source can be `mock`. The screen does not say "Mock planner".

### Mock bridge (browser preview)
In browser preview mode, the frontend's mock bridge handles the request directly. It also checks if the intent matches a known workflow by fuzzy-matching labels against project facts.

## Stage 3: Safety annotation

The backend generates a `PlanReview` alongside the plan:

- **Safety flags:** `DESTRUCTIVE_OPERATION`, `PRIVILEGE_ESCALATION`, `NETWORK_ACCESS`
- **Memory used:** which memory items were included in context
- **Retrieved context:** what context sources were assembled (cwd, project root, workflow names)
- **Ambiguity flags:** (reserved for future use)

## Stage 4: Plan presentation

The `PlanPanel` component renders the full plan:

1. Practice-plan notice, only when the plan is a practice plan
2. Intent (the words the user typed)
3. Editable command
4. What this does, in a sentence
5. Risk, in a sentence
6. A folder-name field, for a high-risk plan only
7. Run Plan, Reject, Save Workflow

## Stage 5: User decision

- **Approve:** the (possibly edited) command is sent to `terminal_execute`
- **Reject:** plan is marked rejected in history, nothing executes
- **Save Workflow:** opens workflow editor to save as reusable workflow
- **Edit:** user modifies command, then approves — history records both generated and executed versions

## Request/response types

```typescript
// Request
type PlannerGeneratePlanRequest = {
  sessionId: string;
  userIntent: string;
  context: PlannerContext;
}

// Response
type PlannerGeneratePlanResponse = {
  plan: CommandPlanPayload;
  review: PlanReviewPayload;
}
```

The `CommandPlanPayload` includes: id, sessionId, source, userIntent, command, cwd, explanation, assumptions, confidence, risk, destructive, requiresConfirmation, touchesFiles, touchesNetwork, escalatesPrivileges, expectedOutput, generatedAt.
