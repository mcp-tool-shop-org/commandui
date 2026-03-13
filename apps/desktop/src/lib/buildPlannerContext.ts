import type { PlannerContext } from "@commandui/api-contract";
import type { HistoryItem, MemoryItem, Workflow, WorkflowRun } from "@commandui/domain";
import { resolveEffectiveMemory } from "@commandui/state";

export type PlannerContextInput = {
  sessionId: string;
  cwd: string;
  shell: string;
  os: "windows" | "macos" | "linux";
  memoryItems: MemoryItem[];
  workflows: Workflow[];
  lastRunByWorkflowId: Record<string, WorkflowRun>;
  recentHistory: HistoryItem[];
};

export function buildPlannerContext(input: PlannerContextInput): PlannerContext {
  const effectiveMemory = resolveEffectiveMemory(input.memoryItems, input.cwd);

  const recentCommands = input.recentHistory
    .filter((h) => h.executedCommand)
    .slice(0, 5)
    .map((h) => h.executedCommand!);

  const relevantWorkflows = input.workflows
    .filter((wf) => !wf.projectRoot || wf.projectRoot === input.cwd)
    .slice(0, 5);

  const projectFacts = relevantWorkflows.map((wf) => {
    const lastRun = input.lastRunByWorkflowId[wf.id];
    const steps = wf.steps?.map((s) => s.command).join(" → ") ?? wf.command;
    const statusSuffix = lastRun ? ` (last run: ${lastRun.status})` : "";
    return {
      kind: "workflow",
      label: wf.label,
      value: `${steps}${statusSuffix}`,
    };
  });

  return {
    sessionId: input.sessionId,
    cwd: input.cwd,
    projectRoot: input.cwd,
    os: input.os,
    shell: input.shell,
    recentCommands,
    memoryItems: effectiveMemory.map((m) => ({
      kind: m.kind,
      key: m.key,
      value: m.value,
      confidence: m.confidence,
    })),
    projectFacts,
  };
}
