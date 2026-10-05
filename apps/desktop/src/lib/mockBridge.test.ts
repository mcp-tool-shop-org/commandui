import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockInvoke, onMockEvent, resetMockBridge } from "./mockBridge";

type Heard = { name: string; payload: unknown };

function listen(): Heard[] & { stop: () => void } {
  const heard: Heard[] = [];
  const names = [
    "session:ready",
    "session:exec_state_changed",
    "terminal:line",
    "terminal:execution_started",
    "terminal:execution_finished",
  ];
  const stops = names.map((name) =>
    onMockEvent(name, (payload) => heard.push({ name, payload })),
  );
  return Object.assign(heard, { stop: () => stops.forEach((stop) => stop()) });
}

describe("mockBridge", () => {
  beforeEach(() => {
    resetMockBridge();
    vi.useFakeTimers();
  });

  afterEach(() => {
    resetMockBridge();
    vi.useRealTimers();
  });

  it("boots a session and then marks it ready", async () => {
    const heard = listen();
    const created = (await mockInvoke<{ session: { id: string; label: string; cwd: string } }>("session_create", {
      request: { label: "", cwd: "/work" },
    }));
    expect(created.session.label).toBe("Session 1");
    expect(created.session.cwd).toBe("/work");
    await vi.advanceTimersByTimeAsync(100);
    expect(heard.map((event) => event.name)).toEqual([
      "session:exec_state_changed",
      "session:ready",
    ]);
    const listed = (await mockInvoke<{ sessions: Array<{ id: string }> }>("session_list"));
    expect(listed.sessions.map((session) => session.id)).toEqual([created.session.id]);
    (await mockInvoke("session_close", { request: { sessionId: created.session.id } }));
    expect((await mockInvoke<{ sessions: unknown[] }>("session_list")).sessions).toEqual([]);
    (await mockInvoke("session_update_cwd", { request: {} }));
    heard.stop();
  });

  it("streams command output and reports an interrupt", async () => {
    const heard = listen();
    const cases = ["echo hello", "ls", "dir", "pwd", "cd", "git status", "git log", "uname"];
    for (const command of cases) {
      (await mockInvoke("terminal_execute", { request: { sessionId: "s", executionId: command, command } }));
    }
    await vi.advanceTimersByTimeAsync(1_000);
    const finished = heard.filter((event) => event.name === "terminal:execution_finished");
    expect(finished).toHaveLength(cases.length);
    expect(heard.some((event) => event.name === "terminal:line" && String((event.payload as { text: string }).text).includes("hello"))).toBe(true);
    expect(heard.some((event) => event.name === "terminal:line" && String((event.payload as { text: string }).text).includes("README.md"))).toBe(true);
    expect(heard.some((event) => event.name === "terminal:line" && String((event.payload as { text: string }).text).includes("(preview) uname"))).toBe(true);

    heard.length = 0;
    (await mockInvoke("terminal_execute", { request: { sessionId: "s", executionId: "stop-me", command: "echo stay" } }));
    (await mockInvoke("terminal_interrupt", { request: { sessionId: "s" } }));
    await vi.advanceTimersByTimeAsync(1_000);
    const stopped = heard.find((event) => event.name === "terminal:execution_finished");
    expect(stopped?.payload).toMatchObject({ status: "interrupted", exitCode: 130 });
    heard.stop();
  });

  it("matches a known workflow when the intent names it", async () => {
    const matched = (await mockInvoke<{ plan: { command: string; explanation: string; confidence: number }; review: { retrievedContext: string[]; memoryUsed: string[] } }>(
      "planner_generate_plan",
      {
        request: {
          userIntent: "please Ship",
          sessionId: "s",
          context: {
            cwd: "/work",
            projectFacts: [{ kind: "workflow", label: "Ship", value: "echo shipped (last run: success)" }],
            memoryItems: [{ kind: "preferred_cwd", key: "workspace" }],
          },
        },
      },
    ));
    expect(matched.plan.command).toBe("echo shipped");
    expect(matched.plan.explanation).toMatch(/Matched known workflow/);
    expect(matched.plan.confidence).toBe(0.95);
    expect(matched.review.retrievedContext).toContain("workflow:Ship");
    expect(matched.review.memoryUsed).toEqual(["preferred_cwd:workspace"]);

    const plain = (await mockInvoke<{ plan: { command: string; risk: string } }>("planner_generate_plan", {
      request: {},
    }));
    expect(plain.plan.command).toContain("do something");
    expect(plain.plan.risk).toBe("low");
  });

  it("stores history, workflows, settings, and memory", async () => {
    (await mockInvoke("history_append", { request: { item: { id: "h1", sessionId: "s", command: "echo a" } } }));
    (await mockInvoke("history_append", { request: { item: { id: "h2", sessionId: "other", command: "echo b" } } }));
    expect((await mockInvoke<{ items: unknown[] }>("history_list", { request: { sessionId: "s" } })).items).toHaveLength(1);
    expect((await mockInvoke<{ items: unknown[] }>("history_list", { request: {} })).items).toHaveLength(2);
    (await mockInvoke("history_update", {
      request: { historyId: "h1", status: "success", exitCode: 0, executedCommand: "echo a", finishedAt: "t", durationMs: 4 },
    }));
    (await mockInvoke("history_update", { request: { historyId: "missing", status: "failure" } }));
    (await mockInvoke("plan_store", { request: {} }));

    const workflow = { id: "w1", label: "Ship", command: "echo a", steps: [{ command: "echo a" }] };
    (await mockInvoke("workflow_add", { request: { workflow } }));
    (await mockInvoke("workflow_add", { request: { workflow: { ...workflow, label: "Ship edited" } } }));
    const listed = (await mockInvoke<{ workflows: Array<{ label: string; stepsJson: string }> }>("workflow_list"));
    expect(listed.workflows).toHaveLength(1);
    expect(listed.workflows[0].label).toBe("Ship edited");
    expect(listed.workflows[0].stepsJson).toContain("echo a");
    (await mockInvoke("workflow_delete", { request: { id: "w1" } }));
    (await mockInvoke("workflow_delete", { request: { id: "missing" } }));
    expect((await mockInvoke<{ workflows: unknown[] }>("workflow_list")).workflows).toEqual([]);

    const settings = (await mockInvoke<{ settings: { productMode: string; defaultInputMode: string } }>("settings_get"));
    expect(settings.settings.productMode).toBe("guided");
    expect(settings.settings.defaultInputMode).toBe("ask");
    expect((await mockInvoke("settings_update"))).toEqual({ ok: true });
    expect((await mockInvoke("terminal_resize"))).toEqual({ ok: true });
    expect((await mockInvoke("terminal_write"))).toEqual({ ok: true });

    const added = (await mockInvoke<{ item: { id: string } }>("memory_add", { request: { key: "k", value: "v" } }));
    const suggestion = { id: "sug", status: "pending", proposedKey: "k", proposedValue: "v", kind: "recurring_command", scope: "project" };
    expect((await mockInvoke<{ inserted: boolean }>("memory_store_suggestion", { request: { suggestion } })).inserted).toBe(true);
    expect((await mockInvoke<{ inserted: boolean }>("memory_store_suggestion", { request: { suggestion } })).inserted).toBe(false);
    expect((await mockInvoke<{ suggestions: unknown[] }>("memory_list")).suggestions).toHaveLength(1);

    const accepted = (await mockInvoke<{ createdItem?: { id: string; key: string } }>("memory_accept_suggestion", { request: { suggestionId: "sug" } }));
    expect(accepted.createdItem?.key).toBe("k");
    expect((await mockInvoke<{ resolved: Array<{ status: string }> }>("memory_list_resolved_suggestions")).resolved[0].status).toBe("accepted");
    expect((await mockInvoke("memory_accept_suggestion", { request: { suggestionId: "missing" } }))).toEqual({ ok: true });

    (await mockInvoke("memory_store_suggestion", { request: { suggestion: { id: "other", status: "pending" } } }));
    (await mockInvoke("memory_dismiss_suggestion", { request: { suggestionId: "other" } }));
    (await mockInvoke("memory_dismiss_suggestion", { request: { suggestionId: "missing" } }));
    const stored = (await mockInvoke<{ items: Array<{ id: string }> }>("memory_list"));
    const ids = stored.items.map((entry) => entry.id);
    expect([...ids].sort()).toEqual([added.item.id, accepted.createdItem?.id].sort());
    for (const id of ids) {
      (await mockInvoke("memory_delete", { request: { memoryId: id } }));
    }
    (await mockInvoke("memory_delete", { request: { memoryId: "missing" } }));
    expect((await mockInvoke<{ items: unknown[] }>("memory_list")).items).toEqual([]);
  });

  it("warns and succeeds for a command it does not know", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect((await mockInvoke("not_a_command"))).toEqual({ ok: true });
    expect(warn).toHaveBeenCalledWith("[mock-bridge] Unknown command: not_a_command");
    warn.mockRestore();
  });

  it("stops delivering events after the caller unsubscribes", async () => {
    const heard: unknown[] = [];
    const stop = onMockEvent("session:ready", (payload) => heard.push(payload));
    stop();
    (await mockInvoke("session_create", { request: { label: "A" } }));
    await vi.advanceTimersByTimeAsync(100);
    expect(heard).toEqual([]);
  });
});
