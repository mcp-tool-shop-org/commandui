import type { SessionSummary } from "@commandui/domain";

export type SessionCreateRequest = {
  label?: string;
  cwd?: string;
  shell?: string;
};

export type SessionCreateResponse = {
  session: SessionSummary;
};

export type SessionListResponse = {
  sessions: SessionSummary[];
};

export type SessionGetActiveResponse = {
  activeSessionId: string | null;
};

export type SessionCloseRequest = {
  sessionId: string;
};

export type SessionCloseResponse = {
  ok: boolean;
};

export type SessionCwdUpdateRequest = {
  sessionId: string;
  cwd: string;
};

export type SessionCwdUpdateResponse = {
  ok: boolean;
};
