export type PlanStoreRequest = {
  plan: {
    id: string;
    sessionId: string;
    userIntent: string;
    command: string;
    risk: string;
    explanation: string;
    generatedAt: string;
  };
};

export type PlanStoreResponse = {
  ok: boolean;
};
