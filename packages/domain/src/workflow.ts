export type WorkflowStep = {
  command: string;
  label?: string;
};

export type Workflow = {
  id: string;
  label: string;
  source: "raw" | "semantic" | "promoted";
  originalIntent?: string;
  command: string;
  steps?: WorkflowStep[];
  projectRoot?: string;
  createdAt: string;
};

/**
 * Row the Rust `Workflow` struct accepts. Serde rename is `stepsJson`.
 * A `steps` array on the same object is ignored and stored as NULL.
 */
export type StoredWorkflow = Workflow & { stepsJson: string | null };

export function toStoredWorkflow(workflow: Workflow): StoredWorkflow {
  return {
    ...workflow,
    stepsJson: workflow.steps ? JSON.stringify(workflow.steps) : null,
  };
}

type WorkflowRow = Workflow & { stepsJson?: string | null };

/** Restore `steps` from the string column a `workflow_list` row actually carries. */
export function hydrateWorkflow(row: WorkflowRow): Workflow {
  const { stepsJson, ...workflow } = row;
  if (workflow.steps && workflow.steps.length > 0) return workflow;
  const steps = parseStepsJson(stepsJson);
  if (!steps) return workflow;
  return { ...workflow, steps };
}

function parseStepsJson(stepsJson: string | null | undefined): WorkflowStep[] | undefined {
  if (!stepsJson) return undefined;
  let parsed: unknown;
  try {
    parsed = JSON.parse(stepsJson);
  } catch {
    return undefined;
  }
  if (!Array.isArray(parsed)) return undefined;
  const steps: WorkflowStep[] = [];
  for (const item of parsed) {
    if (typeof item === "string") {
      const command = item.trim();
      if (command) steps.push({ command });
      continue;
    }
    if (!item || typeof item !== "object" || !("command" in item)) continue;
    const command = (item as { command?: unknown }).command;
    if (typeof command !== "string" || command.length === 0) continue;
    const label = (item as { label?: unknown }).label;
    const step: WorkflowStep = { command };
    if (typeof label === "string") step.label = label;
    steps.push(step);
  }
  return steps.length > 0 ? steps : undefined;
}

// --- Workflow Run (Phase 6C) ---

export type WorkflowRunStatus = "running" | "success" | "failed" | "interrupted";
export type WorkflowStepRunStatus = "pending" | "running" | "success" | "failed" | "interrupted" | "skipped";

export type WorkflowStepRun = {
  index: number;
  command: string;
  label?: string;
  status: WorkflowStepRunStatus;
  historyItemId?: string;
  startedAt?: number;
  finishedAt?: number;
};

export type WorkflowRun = {
  id: string;
  workflowId: string;
  workflowName: string;
  startedAt: number;
  finishedAt?: number;
  status: WorkflowRunStatus;
  currentStepIndex: number;
  steps: WorkflowStepRun[];
};
