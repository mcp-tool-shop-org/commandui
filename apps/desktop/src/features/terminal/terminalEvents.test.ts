import { beforeEach, describe, expect, it, vi } from "vitest";

const listeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  }),
}));

import {
  subscribeToExecStateChanged,
  subscribeToExecutionFinished,
  subscribeToExecutionStarted,
  subscribeToSessionCwdChanged,
  subscribeToSessionReady,
  subscribeToTerminalLines,
} from "./terminalEvents";

describe("terminal event subscriptions", () => {
  beforeEach(() => {
    listeners.clear();
  });

  it("forwards each backend event and stops when the caller unsubscribes", async () => {
    const line = vi.fn();
    const started = vi.fn();
    const finished = vi.fn();
    const cwd = vi.fn();
    const ready = vi.fn();
    const exec = vi.fn();

    const unlisten = await Promise.all([
      subscribeToTerminalLines(line),
      subscribeToExecutionStarted(started),
      subscribeToExecutionFinished(finished),
      subscribeToSessionCwdChanged(cwd),
      subscribeToSessionReady(ready),
      subscribeToExecStateChanged(exec),
    ]);

    listeners.get("terminal:line")?.({ payload: { sessionId: "s", text: "hi" } });
    listeners.get("terminal:execution_started")?.({ payload: { execution: { id: "e" } } });
    listeners.get("terminal:execution_finished")?.({ payload: { executionId: "e", status: "success" } });
    listeners.get("session:cwd_changed")?.({ payload: { sessionId: "s", cwd: "/work" } });
    listeners.get("session:ready")?.({ payload: { sessionId: "s" } });
    listeners.get("session:exec_state_changed")?.({ payload: { sessionId: "s", execState: "ready" } });

    expect(line).toHaveBeenCalledWith({ sessionId: "s", text: "hi" });
    expect(started).toHaveBeenCalledWith({ execution: { id: "e" } });
    expect(finished).toHaveBeenCalledWith({ executionId: "e", status: "success" });
    expect(cwd).toHaveBeenCalledWith({ sessionId: "s", cwd: "/work" });
    expect(ready).toHaveBeenCalledWith({ sessionId: "s" });
    expect(exec).toHaveBeenCalledWith({ sessionId: "s", execState: "ready" });

    unlisten.forEach((stop) => stop());
    listeners.get("terminal:line")?.({ payload: { sessionId: "s", text: "later" } });
    expect(line).toHaveBeenCalledOnce();
  });
});
