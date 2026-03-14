# Memory Detectors

Memory detectors are algorithms that analyze your command history to surface patterns. They run on boot and after each command execution.

## Detector A: Preferred Working Directory

**Function:** `detectPreferredCwd`

Identifies directories where you spend significant time.

**Trigger:** 5+ command executions across 2+ sessions in the same directory.

**Output:** a suggestion with kind `preferred_cwd`, key = the directory path, confidence 0.70–0.95.

**Confidence scaling:** base 0.70, increases with execution count. More executions in more sessions = higher confidence, capped at 0.95.

**Why it matters:** the planner uses preferred CWDs to set context. If you always work in `~/projects/myapp`, the planner knows to interpret "run tests" in that context.

## Detector B: Recurring Commands

**Function:** `detectRecurringCommands`

Identifies commands you run frequently.

**Trigger:** 4+ executions of the same command family. Command families are normalized — `npm test`, `npm run test`, and `npm t` may be grouped if they share a prefix.

**Output:** a suggestion with kind `recurring_command`, key = the command, confidence 0.60–0.90.

**Confidence scaling:** base 0.60, increases with frequency across sessions. More varied sessions = higher confidence.

**Why it matters:** the planner can suggest familiar tools. If you always use `rg` instead of `grep`, the planner learns to generate `rg` commands.

## Detector C: Workflow Patterns

**Function:** `detectWorkflowPatterns`

Identifies command sequences you repeat — pairs and triples of commands that always appear together in order.

**Trigger:** 3+ occurrences of the same sequence across 2+ sessions.

**Output:** a suggestion with kind `workflow_pattern`, key = the sequence label, confidence 0.65–0.85.

**Triple priority:** when a three-step sequence (A → B → C) is detected, it suppresses the constituent pairs (A → B, B → C) to avoid redundant suggestions.

**Why it matters:** these suggestions can be promoted directly to multi-step workflows, automating your most common sequences.

## Detector coordination

All three detectors run through `runDetectors()`, which:

1. Filters history to successful executions only (failed commands are noise)
2. Runs each detector independently
3. Deduplicates against existing memory items and suggestions
4. Returns new suggestions only

## Suggestion lifecycle

```
History data → Detector runs → Suggestion created (pending)
                                        ↓
                              User sees suggestion card
                                        ↓
                    Accept → MemoryItem created → feeds planner
                    Dismiss → suggestion removed permanently
```

## Testing

Detectors are pure functions with comprehensive unit tests in `packages/domain/src/memoryDetectors.test.ts`. Tests verify threshold behavior, confidence scaling, cross-session requirements, deduplication, and triple-over-pair priority.
