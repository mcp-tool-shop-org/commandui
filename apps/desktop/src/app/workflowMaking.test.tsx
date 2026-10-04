import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HistoryItem, MemorySuggestion, Workflow, WorkflowRun } from "@commandui/domain";
import {
  useComposerStore,
  useExecutionStore,
  useFocusStore,
  useHistoryStore,
  useMemoryStore,
  useSessionStore,
  useSettingsStore,
  useWorkflowRunStore,
  useWorkflowStore,
} from "@commandui/state";
import { resetMockBridge } from "../lib/mockBridge";
import { useModalDialog } from "../lib/useModalDialog";
import { HistoryDrawer } from "../components/HistoryDrawer";
import { WorkflowDrawer } from "../components/WorkflowDrawer";
import { WorkflowEditor } from "../components/WorkflowEditor";
import { AppShell } from "./AppShell";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../components/TerminalPane", async () => {
  const { createElement, forwardRef, useImperativeHandle } = await import("react");
  const Pane = forwardRef(function TerminalPane(_props, ref) {
    useImperativeHandle(ref, () => ({
      write() {},
      replay() {},
      clear() {},
      focus() {},
    }));
    return createElement("div", { "data-testid": "terminal" });
  });
  return { TerminalPane: Pane };
});

const EMPTY =
  "Workflows are saved lists of commands you can run again with one click. Make one here, or save commands from History.";

function resetStores() {
  useHistoryStore.getState().clearHistory();
  useSessionStore.setState({ sessions: [], activeSessionId: null });
  useMemoryStore.setState({ items: [], suggestions: [] });
  useWorkflowStore.setState({ items: [] });
  useWorkflowRunStore.setState({ activeRun: null, lastRunByWorkflowId: {} });
  useSettingsStore.setState({
    productMode: "classic",
    reducedClutter: false,
    simplifiedSummaries: false,
    plannerModel: "qwen2.5:14b",
    plannerEndpoint: "http://localhost:11434",
    defaultInputMode: "ask",
  });
  useComposerStore.setState({ inputValue: "", inputMode: "ask" });
  useExecutionStore.setState({
    activeExecutionId: null,
    lastExecutionId: null,
    executionStatus: "idle",
    sessionExecStates: {},
  });
  useFocusStore.setState({ currentZone: null, previousZone: null });
}

async function renderReadyShell() {
  localStorage.setItem("commandui.welcome.showAtStartup", "false");
  render(<AppShell />);
  await waitFor(() => {
    expect(screen.getByPlaceholderText("Describe what you want to do…")).toBeEnabled();
  });
}

function press(user: ReturnType<typeof userEvent.setup>, name: string) {
  const button = screen.getByRole("button", { name });
  button.focus();
  return user.keyboard("{Enter}");
}

describe("workflow editor and dates", () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("adds a step from the keyboard and will not save a blank one", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(
      <WorkflowEditor initialLabel="" initialSteps={[""]} onConfirm={onConfirm} onCancel={() => {}} />,
    );
    expect(screen.getByRole("dialog", { name: "New workflow" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create workflow" })).toBeDisabled();
    await user.type(screen.getByLabelText("Name"), "Ship");
    await user.type(screen.getByLabelText("Step 1"), "echo hi");
    screen.getByRole("button", { name: "Add step" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(screen.getByLabelText("Step 2")).toHaveFocus());
    expect(screen.getByRole("button", { name: "Create workflow" })).toBeDisabled();
    await user.keyboard("echo there");
    screen.getByRole("button", { name: "Create workflow" }).focus();
    await user.keyboard("{Enter}");
    expect(onConfirm).toHaveBeenCalledWith("Ship", ["echo hi", "echo there"]);
  });

  it("shows 2 days ago, with the full date for hover and screen readers", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-04T12:00:00.000Z"));
    const createdAt = "2026-10-02T12:00:00.000Z";
    const full = new Date(createdAt).toLocaleString();
    const run: WorkflowRun = {
      id: "r",
      workflowId: "w",
      workflowName: "Ship",
      startedAt: Date.parse(createdAt),
      finishedAt: Date.parse(createdAt),
      status: "success",
      currentStepIndex: 0,
      steps: [],
    };
    render(
      <WorkflowDrawer
        isOpen
        workflows={[{
          id: "w",
          label: "Ship",
          source: "raw",
          command: "echo hi",
          steps: [{ command: "echo hi" }],
          createdAt,
        }]}
        lastRunByWorkflowId={{ w: run }}
        expandedRunWorkflowId={null}
        onClose={() => {}}
        onRun={() => {}}
        onExpandRun={() => {}}
        onRetryStep={() => {}}
        onCopyCommand={() => {}}
        onViewHistoryItem={() => {}}
      />,
    );
    expect(screen.getAllByText("2 days ago")).toHaveLength(2);
    const stamps = screen.getAllByTitle(full);
    expect(stamps).toHaveLength(2);
    for (const stamp of stamps) {
      expect(stamp.querySelector(".visually-hidden")?.textContent).toContain(full);
    }
  });

  it("keeps the empty-state button, and that button is the one that starts a workflow", async () => {
    const user = userEvent.setup();
    const onNew = vi.fn();
    render(
      <WorkflowDrawer
        isOpen
        workflows={[]}
        lastRunByWorkflowId={{}}
        expandedRunWorkflowId={null}
        onClose={() => {}}
        onRun={() => {}}
        onNew={onNew}
        onExpandRun={() => {}}
        onRetryStep={() => {}}
        onCopyCommand={() => {}}
        onViewHistoryItem={() => {}}
      />,
    );
    const empty = screen.getByText(EMPTY).parentElement;
    expect(empty).toBeTruthy();
    const button = within(empty as HTMLElement).getByRole("button", { name: "New workflow" });
    button.focus();
    await user.keyboard("{Enter}");
    expect(onNew).toHaveBeenCalledOnce();
  });
});

describe("history selection", () => {
  const older: HistoryItem = {
    id: "old",
    sessionId: "s1",
    source: "raw",
    userInput: "echo old",
    executedCommand: "echo old",
    status: "success",
    createdAt: "2026-10-01T00:00:00.000Z",
  };
  const newer: HistoryItem = {
    id: "new",
    sessionId: "s1",
    source: "raw",
    userInput: "echo new",
    executedCommand: "echo new",
    status: "success",
    createdAt: "2026-10-03T00:00:00.000Z",
  };

  afterEach(() => cleanup());

  it("selects the last commands and saves them oldest first", async () => {
    const user = userEvent.setup();
    const onSaveSelected = vi.fn();
    render(
      <HistoryDrawer
        isOpen
        items={[newer, older]}
        allItems={[newer, older]}
        sessions={[{ id: "s1", label: "Notes", cwd: "~/projects", shell: "pwsh", status: "active", createdAt: newer.createdAt, lastActiveAt: newer.createdAt }]}
        activeSessionId="s1"
        onClose={() => {}}
        onRerun={() => {}}
        onReopenPlan={() => {}}
        onSaveWorkflow={() => {}}
        onSaveSelected={onSaveSelected}
        onCopyCommand={() => {}}
      />,
    );
    screen.getByLabelText("Last commands").focus();
    await user.keyboard("{Control>}a{/Control}2");
    screen.getByRole("button", { name: "Select last" }).focus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("checkbox", { name: "Select echo new" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Select echo old" })).toBeChecked();
    screen.getByRole("button", { name: "Save as workflow" }).focus();
    await user.keyboard("{Enter}");
    expect(onSaveSelected).toHaveBeenCalledWith([older, newer]);
  });
});

function UndoCycle() {
  const ref = useModalDialog(true, () => {});
  return (
    <div ref={ref} role="dialog" aria-modal="true" aria-label="Example">
      <button type="button">First</button>
      <button type="button">Last</button>
    </div>
  );
}

describe("undo stays on the keyboard path", () => {
  afterEach(() => cleanup());

  it("tabs from the dialog to Undo and back", async () => {
    const user = userEvent.setup();
    render(
      <>
        <UndoCycle />
        <div className="undo-bar">
          <button type="button">Undo</button>
        </div>
      </>,
    );
    screen.getByRole("button", { name: "Last" }).focus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Undo" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "First" })).toHaveFocus();
  });
});

describe("making a workflow from the shell", () => {
  beforeEach(() => {
    resetStores();
    resetMockBridge();
    localStorage.clear();
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  afterEach(() => {
    cleanup();
    resetMockBridge();
    resetStores();
    localStorage.clear();
  });

  it("creates, edits, runs, and undoes a delete from the keyboard", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    await press(user, "Workflows");
    expect(screen.getByText(EMPTY)).toBeInTheDocument();
    const empty = screen.getByText(EMPTY).parentElement as HTMLElement;
    within(empty).getByRole("button", { name: "New workflow" }).focus();
    await user.keyboard("{Enter}");

    const editor = screen.getByRole("dialog", { name: "New workflow" });
    const name = within(editor).getByLabelText("Name");
    await waitFor(() => expect(name).toHaveFocus());
    await user.keyboard("Ship notes");
    await user.tab();
    expect(within(editor).getByLabelText("Step 1")).toHaveFocus();
    await user.keyboard("echo hello");
    await user.tab();
    expect(within(editor).getByRole("button", { name: "Add step" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(within(editor).getByLabelText("Step 2")).toHaveFocus());
    await user.keyboard("echo there");
    within(editor).getByRole("button", { name: "Create workflow" }).focus();
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "New workflow" })).not.toBeInTheDocument();
    });
    const created = useWorkflowStore.getState().items.find((item) => item.label === "Ship notes");
    expect(created?.steps).toEqual([{ command: "echo hello" }, { command: "echo there" }]);
    expect(created?.command).toBe("echo hello && echo there");
    const createdAt = created?.createdAt;
    const createdId = created?.id;

    await press(user, "Edit");
    const edit = screen.getByRole("dialog", { name: "Edit workflow" });
    const editName = within(edit).getByLabelText("Name");
    await waitFor(() => expect(editName).toHaveFocus());
    await user.clear(editName);
    await user.type(editName, "Rename ship");
    within(edit).getByRole("button", { name: "Save changes" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Edit workflow" })).not.toBeInTheDocument();
    });
    const edited = useWorkflowStore.getState().items.find((item) => item.id === createdId);
    expect(edited?.label).toBe("Rename ship");
    expect(edited?.createdAt).toBe(createdAt);
    expect(edited?.steps).toEqual([{ command: "echo hello" }, { command: "echo there" }]);
    expect(useWorkflowStore.getState().items).toHaveLength(1);

    await press(user, "Run");
    expect(screen.queryByRole("dialog", { name: "Run this workflow here?" })).not.toBeInTheDocument();
    await waitFor(() => {
      const run = Object.values(useWorkflowRunStore.getState().lastRunByWorkflowId)[0];
      expect(run?.status).toBe("success");
    }, { timeout: 8000 });
    expect(screen.getByText(/Finished\./)).toBeInTheDocument();

    await press(user, "Workflows");
    await press(user, "Delete");
    expect(screen.getByRole("dialog", { name: "Delete this workflow?" })).toBeInTheDocument();
    await user.tab();
    expect(screen.getByRole("button", { name: "Delete workflow" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(useWorkflowStore.getState().items).toHaveLength(0);
    });
    const undo = screen.getByRole("button", { name: "Undo" });
    undo.focus();
    await waitFor(() => expect(undo).toHaveFocus());
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(useWorkflowStore.getState().items.some((item) => item.id === createdId)).toBe(true);
    });
    expect(screen.getByText("Rename ship")).toBeInTheDocument();
  });

  it("stores the plan command as a step", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "list the files");
    await user.click(screen.getByRole("button", { name: "Draft plan" }));
    const save = await screen.findByRole("button", { name: "Save Workflow" });
    save.focus();
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(useWorkflowStore.getState().items.length).toBeGreaterThan(0);
    });
    const workflow = useWorkflowStore.getState().items[0] as Workflow;
    expect(workflow.steps).toEqual([{ command: workflow.command }]);
    expect(workflow.command.length).toBeGreaterThan(0);
  });

  it("opens the editor with the last commands in the order they ran", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    const sessionId = useSessionStore.getState().activeSessionId ?? "";
    useHistoryStore.getState().appendHistoryItem({
      id: "old",
      sessionId,
      source: "raw",
      userInput: "echo old",
      executedCommand: "echo old",
      status: "success",
      createdAt: "2026-10-01T00:00:00.000Z",
    });
    useHistoryStore.getState().appendHistoryItem({
      id: "new",
      sessionId,
      source: "raw",
      userInput: "echo new",
      executedCommand: "echo new",
      status: "success",
      createdAt: "2026-10-03T00:00:00.000Z",
    });
    await press(user, "History");
    await press(user, "Select last");
    await press(user, "Save as workflow");
    const editor = screen.getByRole("dialog", { name: "New workflow" });
    expect(within(editor).getByLabelText("Step 1")).toHaveValue("echo old");
    expect(within(editor).getByLabelText("Step 2")).toHaveValue("echo new");
  });

  it("still shows a pattern suggestion when reduced clutter is on", async () => {
    await renderReadyShell();
    useSettingsStore.getState().setReducedClutter(true);
    const suggestion: MemorySuggestion = {
      id: "sug-1",
      scope: "global",
      kind: "workflow_pattern",
      label: "Repeated ship steps",
      proposedKey: "Ship",
      proposedValue: "[\"echo hi\"]",
      confidence: 0.9,
      derivedFromHistoryIds: ["h1", "h2", "h3"],
      status: "pending",
      createdAt: "2026-10-04T00:00:00.000Z",
    };
    useMemoryStore.getState().setMemorySuggestions([suggestion]);
    expect(await screen.findByText("Repeated ship steps")).toBeInTheDocument();
  });
});
