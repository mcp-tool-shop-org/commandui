export type TerminalExecuteRequest = {
  executionId: string;
  sessionId: string;
  command: string;
  source: "raw" | "semantic";
  linkedPlanId?: string;
  cwd?: string;
  env?: Record<string, string>;
};

export type ExecutionSummary = {
  id: string;
  sessionId: string;
  command: string;
  source: "raw" | "semantic";
  linkedPlanId?: string;
  status: "queued" | "running" | "success" | "failure";
  startedAt: string;
  finishedAt?: string;
  exitCode?: number;
};

export type TerminalExecuteResponse = {
  execution: ExecutionSummary;
};

export type TerminalOutputLine = {
  id: string;
  sessionId: string;
  kind: "stdin" | "stdout" | "stderr" | "system";
  text: string;
  timestamp: string;
};

export type TerminalResizeRequest = {
  sessionId: string;
  cols: number;
  rows: number;
};

export type TerminalResizeResponse = {
  ok: boolean;
};

export type TerminalWriteRequest = {
  sessionId: string;
  data: string;
};

export type TerminalWriteResponse = {
  ok: boolean;
};
