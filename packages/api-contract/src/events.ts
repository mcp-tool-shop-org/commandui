export type TerminalLineEvent = {
  id: string;
  sessionId: string;
  executionId?: string;
  /** "notice" is a line the runtime wrote (the shell exited):
   *  not shell output, so a consumer that draws shell output into a full-screen
   *  app must not draw it there. */
  kind: "stdin" | "stdout" | "stderr" | "system" | "notice";
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
  status: "success" | "failure" | "interrupted" | "unknown";
  /** False when exitCode was invented or does not describe the outcome. */
  exitKnown?: boolean;
  /** exit_unknown, shell_exited, or input_not_accepted when the runtime knows why. */
  reason?: string | null;
};

export type SessionCwdChangedEvent = {
  sessionId: string;
  cwd: string;
};

export type SessionReadyEvent = {
  sessionId: string;
  cwd: string;
};

export type SessionExecState =
  | "booting"
  | "ready"
  | "running"
  | "interrupting"
  | "desynced"
  | "userRunning";

export type SessionExecStateChangedEvent = {
  sessionId: string;
  execState: SessionExecState;
  changedAt: string;
};
