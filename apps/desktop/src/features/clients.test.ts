import { afterEach, describe, expect, it } from "vitest";
import { resetMockBridge } from "../lib/mockBridge";
import { memoryAcceptSuggestion, memoryAdd, memoryDelete, memoryDismissSuggestion, memoryList, memoryListResolvedSuggestions, memoryStoreSuggestion } from "./memory/memoryClient";
import { historyAppend, historyList, historyUpdate, planStore, settingsGet, settingsUpdate, workflowAdd, workflowDelete, workflowList } from "./persistence/persistenceClient";
import { generatePlan } from "./planner/plannerClient";
import { closeSession, createSession, executeCommand, interruptTerminal, listSessions, resizeTerminal, resyncTerminal, writeTerminal } from "./terminal/terminalClient";

describe("feature clients on the mock bridge", () => {
  afterEach(() => {
    resetMockBridge();
  });

  it("sends each command the shell uses and reads the mock's answer", async () => {
    const created = await createSession({ label: "One", cwd: "/work" });
    expect(created.session.label).toBe("One");
    const listed = await listSessions();
    expect(listed.sessions).toHaveLength(1);

    const execution = await executeCommand({
      executionId: "e1",
      sessionId: created.session.id,
      command: "echo hi",
      source: "raw",
    });
    expect(execution.execution.command).toBe("echo hi");
    expect(execution.execution.id).toBe("e1");
    expect(await interruptTerminal({ sessionId: created.session.id })).toEqual({ ok: true });
    expect(await resyncTerminal({ sessionId: created.session.id })).toEqual({ ok: true });
    expect(await resizeTerminal({ sessionId: created.session.id, cols: 80, rows: 24 })).toEqual({ ok: true });
    expect(await writeTerminal({ sessionId: created.session.id, data: "a" })).toEqual({ ok: true });

    const plan = await generatePlan({
      sessionId: created.session.id,
      userIntent: "say hi",
      context: {
        sessionId: created.session.id,
        cwd: "/work",
        os: "windows",
        shell: "pwsh",
        recentCommands: [],
        memoryItems: [],
        projectFacts: [],
      },
    });
    expect(plan.plan).not.toBeNull();
    expect(plan.plan!.userIntent).toBe("say hi");

    await historyAppend({
      item: {
        id: "h1",
        sessionId: created.session.id,
        source: "raw",
        userInput: "echo hi",
        status: "planned",
        createdAt: "2026-10-04T00:00:00Z",
      },
    });
    expect((await historyList({ sessionId: created.session.id })).items).toHaveLength(1);
    expect(await historyUpdate({ historyId: "h1", status: "success" })).toEqual({ ok: true });
    expect(await planStore({ plan: plan.plan! })).toEqual({ ok: true });

    await workflowAdd({
      workflow: {
        id: "w1",
        label: "Hi",
        source: "raw",
        command: "echo hi",
        steps: [{ command: "echo hi" }],
        createdAt: "2026-10-04T00:00:00Z",
      },
    });
    const workflows = await workflowList();
    expect(workflows.workflows[0].steps).toEqual([{ command: "echo hi" }]);
    expect(await workflowDelete({ id: "w1" })).toEqual({ ok: true });

    const settings = await settingsGet();
    expect(settings.settings?.productMode).toBe("guided");
    expect(await settingsUpdate({ settings: { productMode: "classic" } })).toEqual({ ok: true });

    await memoryAdd({
      item: {
        id: "m-seed",
        scope: "global",
        kind: "preferred_cwd",
        key: "workspace",
        value: "/work",
        confidence: 0.5,
        source: "manual",
        createdAt: "2026-10-04T00:00:00Z",
        updatedAt: "2026-10-04T00:00:00Z",
      },
    });
    const suggestion = {
      id: "sug",
      scope: "global" as const,
      kind: "recurring_command" as const,
      label: "often",
      proposedKey: "echo hi",
      proposedValue: "echo hi",
      confidence: 0.7,
      derivedFromHistoryIds: [],
      status: "pending" as const,
      createdAt: "2026-10-04T00:00:00Z",
    };
    await memoryStoreSuggestion({ suggestion });
    expect((await memoryList()).suggestions).toHaveLength(1);
    await memoryStoreSuggestion({ suggestion });
    expect((await memoryList()).suggestions).toHaveLength(1);
    expect((await memoryAcceptSuggestion({ suggestionId: "sug" })).createdItem?.value).toBe("echo hi");
    await memoryDismissSuggestion({ suggestionId: "sug" });
    expect((await memoryListResolvedSuggestions()).resolved.length).toBeGreaterThan(0);
    const stored = await memoryList();
    expect(await memoryDelete({ memoryId: stored.items[0].id })).toEqual({ ok: true });

    expect(await closeSession({ sessionId: created.session.id })).toEqual({ ok: true });
    expect((await listSessions()).sessions).toEqual([]);
  });
});
