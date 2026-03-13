import { describe, it, expect } from "vitest";
import type { HistoryItem } from "./history";
import type { MemoryItem, MemorySuggestion } from "./memory";
import {
  normalizeCommand,
  detectPreferredCwd,
  detectRecurringCommands,
  detectWorkflowPatterns,
  runDetectors,
} from "./memoryDetectors";
import type { DetectorInput } from "./memoryDetectors";

// --- Helpers ---

function makeHistory(
  overrides: Partial<HistoryItem> & { id: string },
): HistoryItem {
  return {
    sessionId: "s1",
    source: "raw",
    userInput: "test",
    status: "success",
    createdAt: "2026-03-13T10:00:00Z",
    ...overrides,
  };
}

function emptyInput(
  history: HistoryItem[],
  overrides?: Partial<DetectorInput>,
): DetectorInput {
  return {
    history,
    existingSuggestions: [],
    existingMemory: [],
    ...overrides,
  };
}

// --- normalizeCommand ---

describe("normalizeCommand", () => {
  it("extracts family from two-word command", () => {
    const r = normalizeCommand("git status");
    expect(r.family).toBe("git status");
    expect(r.full).toBe("git status");
  });

  it("extracts family from command with args", () => {
    const r = normalizeCommand("pnpm test --watch --filter api");
    expect(r.family).toBe("pnpm test");
    expect(r.full).toBe("pnpm test --watch --filter api");
  });

  it("trims whitespace and collapses spaces", () => {
    const r = normalizeCommand("  git   status  ");
    expect(r.family).toBe("git status");
    expect(r.full).toBe("git status");
  });

  it("handles single-word command", () => {
    const r = normalizeCommand("ls");
    expect(r.family).toBe("ls");
    expect(r.full).toBe("ls");
  });

  it("handles empty string", () => {
    const r = normalizeCommand("");
    expect(r.family).toBe("");
    expect(r.full).toBe("");
  });
});

// --- detectPreferredCwd ---

describe("detectPreferredCwd", () => {
  it("suggests when threshold met (5 executions, 2 sessions)", () => {
    const history = [
      makeHistory({ id: "1", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "2", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "3", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "4", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "5", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
    ];
    const result = detectPreferredCwd(emptyInput(history));
    expect(result).toHaveLength(1);
    expect(result[0]!.kind).toBe("preferred_cwd");
    expect(result[0]!.proposedValue).toBe("/proj");
    expect(result[0]!.derivedFromHistoryIds).toHaveLength(5);
  });

  it("skips when below execution threshold", () => {
    const history = [
      makeHistory({ id: "1", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "2", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
    ];
    const result = detectPreferredCwd(emptyInput(history));
    expect(result).toHaveLength(0);
  });

  it("skips when below session threshold", () => {
    const history = Array.from({ length: 6 }, (_, i) =>
      makeHistory({ id: `${i}`, sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
    );
    const result = detectPreferredCwd(emptyInput(history));
    expect(result).toHaveLength(0);
  });

  it("skips dismissed suggestions", () => {
    const history = [
      makeHistory({ id: "1", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "2", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "3", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "4", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "5", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
    ];
    const dismissed: MemorySuggestion = {
      id: "d1",
      scope: "global",
      kind: "preferred_cwd",
      label: "dismissed",
      proposedKey: "workspace",
      proposedValue: "/proj",
      confidence: 0.8,
      derivedFromHistoryIds: [],
      status: "dismissed",
      createdAt: "2026-03-13T10:00:00Z",
    };
    const result = detectPreferredCwd(
      emptyInput(history, { existingSuggestions: [dismissed] }),
    );
    expect(result).toHaveLength(0);
  });

  it("skips already accepted memory", () => {
    const history = [
      makeHistory({ id: "1", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "2", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "3", sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "4", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
      makeHistory({ id: "5", sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
    ];
    const accepted: MemoryItem = {
      id: "m1",
      scope: "global",
      kind: "preferred_cwd",
      key: "workspace",
      value: "/proj",
      confidence: 0.9,
      source: "accepted",
      createdAt: "2026-03-13T10:00:00Z",
      updatedAt: "2026-03-13T10:00:00Z",
    };
    const result = detectPreferredCwd(
      emptyInput(history, { existingMemory: [accepted] }),
    );
    expect(result).toHaveLength(0);
  });

  it("ignores failed executions", () => {
    const history = Array.from({ length: 6 }, (_, i) =>
      makeHistory({
        id: `${i}`,
        sessionId: i < 3 ? "s1" : "s2",
        cwd: "/proj",
        executedCommand: "ls",
        status: "failure",
      }),
    );
    const result = detectPreferredCwd(emptyInput(history));
    expect(result).toHaveLength(0);
  });
});

// --- detectRecurringCommands ---

describe("detectRecurringCommands", () => {
  it("suggests when command family threshold met", () => {
    const history = [
      makeHistory({ id: "1", executedCommand: "pnpm test" }),
      makeHistory({ id: "2", executedCommand: "pnpm test --watch" }),
      makeHistory({ id: "3", executedCommand: "pnpm test --filter api" }),
      makeHistory({ id: "4", executedCommand: "pnpm test" }),
    ];
    const result = detectRecurringCommands(emptyInput(history));
    expect(result).toHaveLength(1);
    expect(result[0]!.kind).toBe("recurring_command");
    expect(result[0]!.proposedValue).toBe("pnpm test");
  });

  it("skips when below threshold", () => {
    const history = [
      makeHistory({ id: "1", executedCommand: "pnpm test" }),
      makeHistory({ id: "2", executedCommand: "pnpm test" }),
    ];
    const result = detectRecurringCommands(emptyInput(history));
    expect(result).toHaveLength(0);
  });

  it("groups by normalized family", () => {
    const history = [
      makeHistory({ id: "1", executedCommand: "git status" }),
      makeHistory({ id: "2", executedCommand: "git  status" }),
      makeHistory({ id: "3", executedCommand: "git status ." }),
      makeHistory({ id: "4", executedCommand: " git status" }),
    ];
    const result = detectRecurringCommands(emptyInput(history));
    expect(result).toHaveLength(1);
    expect(result[0]!.proposedValue).toBe("git status");
  });

  it("skips dismissed suggestions", () => {
    const history = Array.from({ length: 5 }, (_, i) =>
      makeHistory({ id: `${i}`, executedCommand: "pnpm test" }),
    );
    const dismissed: MemorySuggestion = {
      id: "d1",
      scope: "global",
      kind: "recurring_command",
      label: "dismissed",
      proposedKey: "pnpm test",
      proposedValue: "pnpm test",
      confidence: 0.7,
      derivedFromHistoryIds: [],
      status: "dismissed",
      createdAt: "2026-03-13T10:00:00Z",
    };
    const result = detectRecurringCommands(
      emptyInput(history, { existingSuggestions: [dismissed] }),
    );
    expect(result).toHaveLength(0);
  });
});

// --- detectWorkflowPatterns ---

describe("detectWorkflowPatterns", () => {
  it("detects repeated pairs across sessions", () => {
    const history = [
      // Session 1
      makeHistory({ id: "1a", sessionId: "s1", executedCommand: "git status", createdAt: "2026-03-13T10:00:00Z" }),
      makeHistory({ id: "1b", sessionId: "s1", executedCommand: "pnpm test", createdAt: "2026-03-13T10:01:00Z" }),
      // Session 2
      makeHistory({ id: "2a", sessionId: "s2", executedCommand: "git status", createdAt: "2026-03-13T11:00:00Z" }),
      makeHistory({ id: "2b", sessionId: "s2", executedCommand: "pnpm test", createdAt: "2026-03-13T11:01:00Z" }),
      // Session 3
      makeHistory({ id: "3a", sessionId: "s3", executedCommand: "git status", createdAt: "2026-03-13T12:00:00Z" }),
      makeHistory({ id: "3b", sessionId: "s3", executedCommand: "pnpm test", createdAt: "2026-03-13T12:01:00Z" }),
    ];
    const result = detectWorkflowPatterns(emptyInput(history));
    expect(result).toHaveLength(1);
    expect(result[0]!.kind).toBe("workflow_pattern");
    expect(result[0]!.label).toContain("git status → pnpm test");
  });

  it("skips when below occurrence threshold", () => {
    const history = [
      makeHistory({ id: "1a", sessionId: "s1", executedCommand: "git status", createdAt: "2026-03-13T10:00:00Z" }),
      makeHistory({ id: "1b", sessionId: "s1", executedCommand: "pnpm test", createdAt: "2026-03-13T10:01:00Z" }),
      makeHistory({ id: "2a", sessionId: "s2", executedCommand: "git status", createdAt: "2026-03-13T11:00:00Z" }),
      makeHistory({ id: "2b", sessionId: "s2", executedCommand: "pnpm test", createdAt: "2026-03-13T11:01:00Z" }),
    ];
    const result = detectWorkflowPatterns(emptyInput(history));
    expect(result).toHaveLength(0);
  });

  it("skips when below session threshold", () => {
    const history = [
      makeHistory({ id: "1a", sessionId: "s1", executedCommand: "git status", createdAt: "2026-03-13T10:00:00Z" }),
      makeHistory({ id: "1b", sessionId: "s1", executedCommand: "pnpm test", createdAt: "2026-03-13T10:01:00Z" }),
      makeHistory({ id: "2a", sessionId: "s1", executedCommand: "git status", createdAt: "2026-03-13T10:02:00Z" }),
      makeHistory({ id: "2b", sessionId: "s1", executedCommand: "pnpm test", createdAt: "2026-03-13T10:03:00Z" }),
      makeHistory({ id: "3a", sessionId: "s1", executedCommand: "git status", createdAt: "2026-03-13T10:04:00Z" }),
      makeHistory({ id: "3b", sessionId: "s1", executedCommand: "pnpm test", createdAt: "2026-03-13T10:05:00Z" }),
    ];
    const result = detectWorkflowPatterns(emptyInput(history));
    expect(result).toHaveLength(0);
  });

  it("prefers triples over constituent pairs", () => {
    const makeSeq = (sid: string, base: string) => [
      makeHistory({ id: `${sid}a`, sessionId: sid, executedCommand: "git status", createdAt: `${base}:00Z` }),
      makeHistory({ id: `${sid}b`, sessionId: sid, executedCommand: "pnpm test", createdAt: `${base}:01Z` }),
      makeHistory({ id: `${sid}c`, sessionId: sid, executedCommand: "pnpm build", createdAt: `${base}:02Z` }),
    ];
    const history = [
      ...makeSeq("s1", "2026-03-13T10:00"),
      ...makeSeq("s2", "2026-03-13T11:00"),
      ...makeSeq("s3", "2026-03-13T12:00"),
    ];
    const result = detectWorkflowPatterns(emptyInput(history));
    // Should have the triple, but not the constituent pairs
    const triple = result.find((r) => r.label.includes("git status → pnpm test → pnpm build"));
    expect(triple).toBeDefined();
    const pair = result.find(
      (r) => r.label === "You often run: git status → pnpm test" && !r.label.includes("build"),
    );
    expect(pair).toBeUndefined();
  });

  it("skips dismissed workflow patterns", () => {
    const history = [
      makeHistory({ id: "1a", sessionId: "s1", executedCommand: "git status", createdAt: "2026-03-13T10:00:00Z" }),
      makeHistory({ id: "1b", sessionId: "s1", executedCommand: "pnpm test", createdAt: "2026-03-13T10:01:00Z" }),
      makeHistory({ id: "2a", sessionId: "s2", executedCommand: "git status", createdAt: "2026-03-13T11:00:00Z" }),
      makeHistory({ id: "2b", sessionId: "s2", executedCommand: "pnpm test", createdAt: "2026-03-13T11:01:00Z" }),
      makeHistory({ id: "3a", sessionId: "s3", executedCommand: "git status", createdAt: "2026-03-13T12:00:00Z" }),
      makeHistory({ id: "3b", sessionId: "s3", executedCommand: "pnpm test", createdAt: "2026-03-13T12:01:00Z" }),
    ];
    const dismissed: MemorySuggestion = {
      id: "d1",
      scope: "global",
      kind: "workflow_pattern",
      label: "dismissed",
      proposedKey: "git status → pnpm test",
      proposedValue: JSON.stringify(["git status", "pnpm test"]),
      confidence: 0.7,
      derivedFromHistoryIds: [],
      status: "dismissed",
      createdAt: "2026-03-13T10:00:00Z",
    };
    const result = detectWorkflowPatterns(
      emptyInput(history, { existingSuggestions: [dismissed] }),
    );
    expect(result).toHaveLength(0);
  });
});

// --- runDetectors ---

describe("runDetectors", () => {
  it("returns combined results from all detectors", () => {
    const history = [
      // CWD pattern: 5 executions across 2 sessions
      ...Array.from({ length: 3 }, (_, i) =>
        makeHistory({ id: `cwd-s1-${i}`, sessionId: "s1", cwd: "/proj", executedCommand: "ls" }),
      ),
      ...Array.from({ length: 2 }, (_, i) =>
        makeHistory({ id: `cwd-s2-${i}`, sessionId: "s2", cwd: "/proj", executedCommand: "ls" }),
      ),
      // Command pattern: 4 executions of git status
      makeHistory({ id: "cmd1", executedCommand: "git status" }),
      makeHistory({ id: "cmd2", executedCommand: "git status" }),
      makeHistory({ id: "cmd3", executedCommand: "git status" }),
      makeHistory({ id: "cmd4", executedCommand: "git status" }),
    ];
    const result = runDetectors(emptyInput(history));
    const kinds = result.map((r) => r.kind);
    expect(kinds).toContain("preferred_cwd");
    expect(kinds).toContain("recurring_command");
  });

  it("returns empty when no patterns detected", () => {
    const history = [
      makeHistory({ id: "1", executedCommand: "ls" }),
      makeHistory({ id: "2", executedCommand: "pwd" }),
    ];
    const result = runDetectors(emptyInput(history));
    expect(result).toHaveLength(0);
  });
});
