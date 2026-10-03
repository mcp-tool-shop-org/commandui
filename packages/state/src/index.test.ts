import { describe, it, expect, beforeEach } from "vitest";
import {
  useExecutionStore,
  useFocusStore,
  useHistoryStore,
  useMemoryStore,
  useSettingsStore,
  useSessionStore,
  useWorkflowStore,
  resolveEffectiveMemory,
} from "./index";
import type { MemoryItem, MemorySuggestion } from "@commandui/domain";

describe("ExecutionStore", () => {
  beforeEach(() => {
    useExecutionStore.setState({
      activeExecutionId: null,
      lastExecutionId: null,
      executionStatus: "idle",
    });
  });

  it("sets active execution", () => {
    useExecutionStore.getState().setActiveExecution("exec-1");
    expect(useExecutionStore.getState().activeExecutionId).toBe("exec-1");
  });

  it("sets execution status", () => {
    useExecutionStore.getState().setExecutionStatus("running");
    expect(useExecutionStore.getState().executionStatus).toBe("running");
  });
});

describe("HistoryStore", () => {
  beforeEach(() => {
    useHistoryStore.setState({ items: [] });
  });

  it("appends history items (newest first)", () => {
    const item1 = {
      id: "1",
      sessionId: "s1",
      source: "raw" as const,
      userInput: "ls",
      status: "success" as const,
      createdAt: "2025-01-01",
    };
    const item2 = {
      id: "2",
      sessionId: "s1",
      source: "raw" as const,
      userInput: "pwd",
      status: "success" as const,
      createdAt: "2025-01-02",
    };

    useHistoryStore.getState().appendHistoryItem(item1);
    useHistoryStore.getState().appendHistoryItem(item2);

    const items = useHistoryStore.getState().items;
    expect(items.length).toBe(2);
    expect(items[0].id).toBe("2"); // newest first
  });

  it("updates history item by id", () => {
    useHistoryStore.getState().appendHistoryItem({
      id: "1",
      sessionId: "s1",
      source: "raw",
      userInput: "ls",
      status: "planned",
      createdAt: "2025-01-01",
    });

    useHistoryStore
      .getState()
      .updateHistoryItem("1", { status: "success", exitCode: 0 });
    expect(useHistoryStore.getState().items[0].status).toBe("success");
  });

  it("loads history items (bulk replace)", () => {
    useHistoryStore.getState().appendHistoryItem({
      id: "old",
      sessionId: "s1",
      source: "raw",
      userInput: "stale",
      status: "success",
      createdAt: "2025-01-01",
    });

    useHistoryStore.getState().loadHistory([
      {
        id: "new1",
        sessionId: "s1",
        source: "semantic",
        userInput: "fresh",
        status: "success",
        createdAt: "2025-01-02",
      },
      {
        id: "new2",
        sessionId: "s1",
        source: "raw",
        userInput: "also fresh",
        status: "failure",
        createdAt: "2025-01-03",
      },
    ]);

    const items = useHistoryStore.getState().items;
    expect(items.length).toBe(2);
    expect(items[0].id).toBe("new1");
    expect(items[1].id).toBe("new2");
  });
});

describe("MemoryStore", () => {
  beforeEach(() => {
    useMemoryStore.setState({ items: [], suggestions: [] });
  });

  it("removes suggestion by id", () => {
    useMemoryStore.setState({
      suggestions: [
        {
          id: "s1",
          scope: "global",
          kind: "accepted_substitution",
          label: "test",
          proposedKey: "k",
          proposedValue: "v",
          confidence: 0.8,
          derivedFromHistoryIds: [],
          status: "pending",
          createdAt: "2025-01-01",
        },
        {
          id: "s2",
          scope: "global",
          kind: "accepted_substitution",
          label: "test2",
          proposedKey: "k2",
          proposedValue: "v2",
          confidence: 0.8,
          derivedFromHistoryIds: [],
          status: "pending",
          createdAt: "2025-01-01",
        },
      ],
    });

    useMemoryStore.getState().removeSuggestion("s1");
    expect(useMemoryStore.getState().suggestions.length).toBe(1);
    expect(useMemoryStore.getState().suggestions[0].id).toBe("s2");
  });

  const makeSuggestion = (
    id: string,
    overrides: Partial<MemorySuggestion> = {},
  ): MemorySuggestion => ({
    id,
    scope: "global",
    kind: "accepted_substitution",
    label: id,
    proposedKey: "k",
    proposedValue: "v",
    confidence: 0.8,
    derivedFromHistoryIds: [],
    status: "pending",
    createdAt: "2025-01-01",
    ...overrides,
  });

  const makeItem = (
    id: string,
    overrides: Partial<MemoryItem> = {},
  ): MemoryItem => ({
    id,
    scope: "global",
    kind: "preferred_cwd",
    key: "workspace",
    value: "v",
    confidence: 0.9,
    source: "manual",
    createdAt: "2025-01-01",
    updatedAt: "2025-01-01",
    ...overrides,
  });

  it("setMemorySuggestions dedupes duplicate ids", () => {
    useMemoryStore
      .getState()
      .setMemorySuggestions([
        makeSuggestion("a"),
        makeSuggestion("a", { proposedKey: "other" }),
      ]);
    const result = useMemoryStore.getState().suggestions;
    expect(result).toHaveLength(1);
    expect(result[0].proposedKey).toBe("k");
  });

  it("setMemorySuggestions dedupes duplicate logical keys but keeps distinct ones", () => {
    useMemoryStore.getState().setMemorySuggestions([
      makeSuggestion("a"),
      makeSuggestion("b"), // same scope/root/kind/key/value as a
      makeSuggestion("c", { scope: "project", projectRoot: "/p" }),
      makeSuggestion("d", { scope: "project", projectRoot: "/q" }),
      makeSuggestion("e", { kind: "preferred_cwd" }),
      makeSuggestion("f", { proposedKey: "k2" }),
      makeSuggestion("g", { proposedValue: "v2" }),
    ]);
    expect(useMemoryStore.getState().suggestions.map((s) => s.id)).toEqual([
      "a",
      "c",
      "d",
      "e",
      "f",
      "g",
    ]);
  });

  it("setMemorySuggestions applies a functional updater and dedupes the result", () => {
    useMemoryStore.setState({ suggestions: [makeSuggestion("a")] });
    useMemoryStore
      .getState()
      .setMemorySuggestions((prev) => [
        ...prev,
        makeSuggestion("a"),
        makeSuggestion("b"),
        makeSuggestion("c", { proposedValue: "v3" }),
      ]);
    expect(useMemoryStore.getState().suggestions.map((s) => s.id)).toEqual([
      "a",
      "c",
    ]);
  });

  it("resolveEffectiveMemory lets a project item shadow a global item with the same kind and key", () => {
    const items = [
      makeItem("g1", { value: "global-ws" }),
      makeItem("g2", { kind: "preferred_search_tool", key: "search", value: "rg" }),
      makeItem("p1", {
        scope: "project",
        projectRoot: "/proj",
        value: "project-ws",
      }),
      makeItem("p-other", {
        scope: "project",
        projectRoot: "/other",
        kind: "preferred_search_tool",
        key: "search",
        value: "grep",
      }),
    ];
    const result = resolveEffectiveMemory(items, "/proj");
    expect(result.map((i) => i.id).sort()).toEqual(["g2", "p1"]);
  });
});

describe("SettingsStore", () => {
  it("updates product mode", () => {
    useSettingsStore.getState().setProductMode("guided");
    expect(useSettingsStore.getState().productMode).toBe("guided");
  });
});

describe("SessionStore", () => {
  beforeEach(() => {
    useSessionStore.setState({ sessions: [], activeSessionId: null });
  });

  it("adds session and auto-selects first", () => {
    useSessionStore.getState().addSession({
      id: "s1",
      label: "Session 1",
      cwd: "/tmp",
      shell: "bash",
      status: "active",
      createdAt: "2025-01-01",
      lastActiveAt: "2025-01-01",
    });

    expect(useSessionStore.getState().activeSessionId).toBe("s1");
    expect(useSessionStore.getState().sessions.map((s) => s.id)).toEqual(["s1"]);
  });

  it("keeps the active session when a second session is added", () => {
    const base = {
      cwd: "/tmp",
      shell: "bash",
      status: "active" as const,
      createdAt: "2025-01-01",
      lastActiveAt: "2025-01-01",
    };
    useSessionStore.getState().addSession({ ...base, id: "s1", label: "S1" });
    useSessionStore.getState().addSession({ ...base, id: "s2", label: "S2" });
    expect(useSessionStore.getState().activeSessionId).toBe("s1");
    expect(useSessionStore.getState().sessions.map((s) => s.id)).toEqual(["s1", "s2"]);
  });

  it("removes session and auto-selects next", () => {
    useSessionStore.setState({
      sessions: [
        {
          id: "s1",
          label: "S1",
          cwd: "/",
          shell: "bash",
          status: "active",
          createdAt: "",
          lastActiveAt: "",
        },
        {
          id: "s2",
          label: "S2",
          cwd: "/",
          shell: "bash",
          status: "active",
          createdAt: "",
          lastActiveAt: "",
        },
      ],
      activeSessionId: "s1",
    });

    useSessionStore.getState().removeSession("s1");
    expect(useSessionStore.getState().activeSessionId).toBe("s2");
    expect(useSessionStore.getState().sessions.map((s) => s.id)).toEqual(["s2"]);
  });

  it("removing a non-active session keeps the current active id", () => {
    const mk = (id: string) => ({
      id,
      label: id,
      cwd: "/",
      shell: "bash",
      status: "active" as const,
      createdAt: "",
      lastActiveAt: "",
    });
    useSessionStore.setState({
      sessions: [mk("s1"), mk("s2")],
      activeSessionId: "s1",
    });
    useSessionStore.getState().removeSession("s2");
    expect(useSessionStore.getState().activeSessionId).toBe("s1");
    expect(useSessionStore.getState().sessions.map((s) => s.id)).toEqual(["s1"]);
  });
});

describe("FocusStore", () => {
  beforeEach(() => {
    useFocusStore.setState({ currentZone: null, previousZone: null });
  });

  it("sets focus zone and tracks previous", () => {
    useFocusStore.getState().setFocusZone("composer");
    expect(useFocusStore.getState().currentZone).toBe("composer");
    expect(useFocusStore.getState().previousZone).toBeNull();

    useFocusStore.getState().setFocusZone("terminal");
    expect(useFocusStore.getState().currentZone).toBe("terminal");
    expect(useFocusStore.getState().previousZone).toBe("composer");
  });

  it("restores previous zone", () => {
    useFocusStore.getState().setFocusZone("composer");
    useFocusStore.getState().setFocusZone("drawer");
    useFocusStore.getState().restorePreviousZone();

    expect(useFocusStore.getState().currentZone).toBe("composer");
    expect(useFocusStore.getState().previousZone).toBeNull();
  });
});

describe("WorkflowStore", () => {
  it("adds workflow (newest first)", () => {
    useWorkflowStore.setState({ items: [] });

    useWorkflowStore.getState().addWorkflow({
      id: "w1",
      label: "test",
      source: "raw",
      command: "ls",
      createdAt: "2025-01-01",
    });

    expect(useWorkflowStore.getState().items.length).toBe(1);
    expect(useWorkflowStore.getState().items[0].id).toBe("w1");
  });

  it("puts the later workflow first and ignores a repeated id", () => {
    useWorkflowStore.setState({ items: [] });
    const wf = (id: string) => ({
      id,
      label: id,
      source: "raw" as const,
      command: "ls",
      createdAt: "2025-01-01",
    });
    useWorkflowStore.getState().addWorkflow(wf("w1"));
    useWorkflowStore.getState().addWorkflow(wf("w2"));
    expect(useWorkflowStore.getState().items.map((w) => w.id)).toEqual(["w2", "w1"]);

    useWorkflowStore.getState().addWorkflow(wf("w1"));
    expect(useWorkflowStore.getState().items.map((w) => w.id)).toEqual(["w2", "w1"]);
  });
});
