import { describe, expect, it } from "vitest";
import type { HistoryItem, MemoryItem, Workflow, WorkflowRun } from "@commandui/domain";
import { buildPlannerContext } from "./buildPlannerContext";

function memory(partial: Partial<MemoryItem> & Pick<MemoryItem, "id" | "key" | "value">): MemoryItem {
  return {
    scope: "global",
    kind: "preferred_cwd",
    confidence: 0.8,
    source: "manual",
    createdAt: "2026-10-04T00:00:00Z",
    updatedAt: "2026-10-04T00:00:00Z",
    ...partial,
  };
}

function history(partial: Partial<HistoryItem> & Pick<HistoryItem, "id">): HistoryItem {
  return {
    sessionId: "s",
    source: "raw",
    userInput: partial.executedCommand ?? partial.id,
    status: "success",
    createdAt: "2026-10-04T00:00:00Z",
    ...partial,
  };
}

function workflow(partial: Partial<Workflow> & Pick<Workflow, "id" | "label" | "command">): Workflow {
  return {
    source: "raw",
    createdAt: "2026-10-04T00:00:00Z",
    ...partial,
  };
}

describe("buildPlannerContext", () => {
  it("sends the project memory, the matching workflows, and the five newest commands", () => {
    const cwd = "/work/app";
    const runs: Record<string, WorkflowRun> = {
      ship: {
        id: "r1",
        workflowId: "ship",
        workflowName: "Ship",
        startedAt: 1,
        status: "success",
        currentStepIndex: 1,
        steps: [],
      },
    };
    const recent = [
      history({ id: "h0" }),
      ...Array.from({ length: 6 }, (_, i) =>
        history({ id: `h${i + 1}`, executedCommand: `cmd-${i + 1}` }),
      ),
    ];

    const context = buildPlannerContext({
      sessionId: "s1",
      cwd,
      shell: "pwsh",
      os: "windows",
      memoryItems: [
        memory({ id: "g", key: "workspace", value: "global" }),
        memory({
          id: "p",
          scope: "project",
          projectRoot: cwd,
          key: "workspace",
          value: "project",
        }),
        memory({ id: "other", scope: "project", projectRoot: "/elsewhere", key: "tool", value: "rg" }),
      ],
      workflows: [
        workflow({
          id: "ship",
          label: "Ship",
          command: "echo fallback",
          projectRoot: cwd,
          steps: [{ command: "git status" }, { command: "git diff", label: "diff" }],
        }),
        workflow({ id: "elsewhere", label: "Other", command: "echo no", projectRoot: "/elsewhere" }),
        workflow({ id: "any", label: "Any", command: "echo any" }),
      ],
      lastRunByWorkflowId: runs,
      recentHistory: recent,
    });

    expect(context.sessionId).toBe("s1");
    expect(context.cwd).toBe(cwd);
    expect(context.projectRoot).toBe(cwd);
    expect(context.os).toBe("windows");
    expect(context.shell).toBe("pwsh");
    expect(context.recentCommands).toEqual(["cmd-1", "cmd-2", "cmd-3", "cmd-4", "cmd-5"]);
    expect(context.memoryItems.map((m) => m.value)).toEqual(["project"]);
    expect(context.projectFacts).toEqual([
      { kind: "workflow", label: "Ship", value: "git status → git diff (last run: success)" },
      { kind: "workflow", label: "Any", value: "echo any" },
    ]);
  });
});
