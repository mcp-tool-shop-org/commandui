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

export type PlannerGeneratePlanRequest = {
  sessionId: string;
  userIntent: string;
  context: PlannerContext;
};

export type PlannerGeneratePlanResponse = {
  plan: CommandPlan;
  review: PlanReview;
};
