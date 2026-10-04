import type { CommandPlan, PlanReview } from "@commandui/domain";

export type MemoryItemSummary = {
  kind: string;
  key: string;
  value: string;
  confidence: number;
};

export type ProjectFact = {
  kind: string;
  label: string;
  value: string;
};

export type PlannerContext = {
  sessionId: string;
  cwd: string;
  projectRoot?: string;
  os: "windows" | "macos" | "linux";
  shell: string;
  recentCommands: string[];
  memoryItems: MemoryItemSummary[];
  projectFacts: ProjectFact[];
};

export type PlannerStatusState =
  | "ready"
  | "notInstalled"
  | "notRunning"
  | "modelMissing"
  | "unavailable";

export type PlannerStatus = {
  state: PlannerStatusState;
  model: string;
  endpoint: string;
  headline: string;
  fix: string;
  link: string;
  linkLabel: string;
};

export type PlannerGeneratePlanRequest = {
  sessionId: string;
  userIntent: string;
  context: PlannerContext;
  model?: string;
  endpoint?: string;
  probeOnly?: boolean;
};

export type PlannerGeneratePlanResponse = {
  plan: CommandPlan | null;
  review: PlanReview | null;
  status: PlannerStatus;
};
