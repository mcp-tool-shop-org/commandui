export type SessionSummary = {
  id: string;
  label: string;
  cwd: string;
  shell: string;
  status: "active" | "idle" | "disconnected" | "exited";
  createdAt: string;
  lastActiveAt: string;
};
