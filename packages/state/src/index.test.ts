import { describe, it, expect, beforeEach } from "vitest";
import {
  useExecutionStore,
  useFocusStore,
  useHistoryStore,
  useMemoryStore,
  useSettingsStore,
  useSessionStore,
  useWorkflowStore,
} from "./index";

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
});
