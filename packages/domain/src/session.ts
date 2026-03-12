export type SessionSummary = {
  id: string;
  label: string;
  cwd: string;
  shell: string;
  status: "active" | "idle" | "disconnected";
  createdAt: string;
  lastActiveAt: string;
};
