export type TerminalLineEvent = {
  id: string;
  sessionId: string;
  executionId?: string;
  kind: "stdin" | "stdout" | "stderr" | "system";
  text: string;
  timestamp: string;
};

export type TerminalExecutionStartedEvent = {
  execution: {
    id: string;
    sessionId: string;
    command: string;
    source: "raw" | "semantic";
    status: "running";
    startedAt: string;
  };
};

export type TerminalExecutionFinishedEvent = {
  executionId: string;
  sessionId: string;
  exitCode: number;
  finishedAt: string;
  status: "success" | "failure";
};

export type SessionCwdChangedEvent = {
  sessionId: string;
  cwd: string;
};
