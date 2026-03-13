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
