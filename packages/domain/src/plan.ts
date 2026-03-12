export type CommandPlan = {
  id: string;
  sessionId: string;
  source: "raw" | "semantic";
  userIntent: string;
  command: string;
  cwd?: string;
  env?: Record<string, string>;
  explanation: string;
  assumptions: string[];
  confidence: number;
  risk: "low" | "medium" | "high";
  destructive: boolean;
  requiresConfirmation: boolean;
  touchesFiles: boolean;
  touchesNetwork: boolean;
  escalatesPrivileges: boolean;
  expectedOutput?: string;
  generatedAt: string;
};

export type PlanReview = {
  planId: string;
  ambiguityFlags: string[];
  safetyFlags: string[];
  memoryUsed: string[];
  retrievedContext: string[];
};
