export type HistoryItem = {
  id: string;
  sessionId: string;
  source: "raw" | "semantic";
  userInput: string;
  generatedCommand?: string;
  executedCommand?: string;
  linkedPlanId?: string;
  plannerRequestId?: string;
  status: "planned" | "rejected" | "success" | "failure" | "interrupted" | "unknown";
  exitCode?: number;
  createdAt: string;
};
