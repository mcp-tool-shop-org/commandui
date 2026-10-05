import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";
import type { HistoryItem, MemoryItem, SessionSummary, Workflow } from "@commandui/domain";
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
import { CommandPalette } from "../components/CommandPalette";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HelpDialog } from "../components/HelpDialog";
import { HistoryDrawer } from "../components/HistoryDrawer";
import { MemoryDrawer } from "../components/MemoryDrawer";
import { OutputView } from "../components/OutputView";
import { PlanPanel } from "../components/PlanPanel";
import { SettingsDrawer } from "../components/SettingsDrawer";
import { WelcomeScreen } from "../components/WelcomeScreen";
import { WorkflowDrawer } from "../components/WorkflowDrawer";
import { WorkflowEditor } from "../components/WorkflowEditor";
import { WorkflowRunBanner } from "../components/WorkflowRunBanner";
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

// jsdom does not compute color contrast. Workstream 5 measures the tokens.
const AXE = { rules: { "color-contrast": { enabled: false } } };

async function expectClean(container: HTMLElement) {
  const results = await axe(container, AXE);
  const summary = results.violations
    .map((violation) => `${violation.id}: ${violation.help} (${violation.nodes.map((node) => node.target.join(" ")).join(", ")})`)
    .join("\n");
  expect(summary).toBe("");
}

const session = { id: "s1", label: "Notes" } as unknown as SessionSummary;
const historyItem: HistoryItem = {
  id: "h1",
  sessionId: "s1",
  source: "raw",
  userInput: "echo hello",
  executedCommand: "echo hello",
  status: "success",
  createdAt: "2026-10-04T00:00:00.000Z",
};
const memoryItem: MemoryItem = {
  id: "m1",
  scope: "global",
  kind: "tool_preference",
  key: "editor",
  value: "helix",
  confidence: 1,
  source: "manual",
  createdAt: "2026-10-04T00:00:00.000Z",
  updatedAt: "2026-10-04T00:00:00.000Z",
};
const workflow: Workflow = {
  id: "wf1",
  label: "Ship the notes",
  source: "raw",
  command: "echo hi",
  steps: [{ command: "echo hi" }],
  createdAt: "2026-10-04T00:00:00.000Z",
};

function resetStores() {
  useHistoryStore.getState().clearHistory();
  useSessionStore.setState({ sessions: [], activeSessionId: null });
  useMemoryStore.setState({ items: [], suggestions: [] });
  useWorkflowStore.setState({ items: [] });
  useWorkflowRunStore.setState({ activeRun: null, lastRunByWorkflowId: {} });
  useSettingsStore.setState({
    productMode: "classic",
    fontSize: "md",
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
  const view = render(<AppShell />);
  await waitFor(() => {
    expect(screen.getByPlaceholderText("Describe what you want to do…")).toBeEnabled();
  });
  return view;
}

describe("Workstream 2 accessibility", () => {
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

  it("has no axe violations on the shell and the open screens", async () => {
    const shell = await renderReadyShell();
    await expectClean(shell.container);
    shell.unmount();

    const welcome = render(
      <WelcomeScreen
        onStart={() => {}}
        showAtStartup
        onShowAtStartupChange={() => {}}
        plannerModel="qwen2.5:14b"
      />,
    );
    await expectClean(welcome.container);
    welcome.unmount();

    const history = render(
      <HistoryDrawer
        isOpen
        items={[historyItem]}
        allItems={[historyItem]}
        sessions={[session]}
        activeSessionId="s1"
        onClose={() => {}}
        onRerun={() => {}}
        onReopenPlan={() => {}}
        onSaveWorkflow={() => {}}
        onSaveSelected={() => {}}
        onCopyCommand={() => {}}
      />,
    );
    await expectClean(history.container);
    history.unmount();

    const workflows = render(
      <WorkflowDrawer
        isOpen
        workflows={[workflow]}
        lastRunByWorkflowId={{}}
        expandedRunWorkflowId={null}
        onClose={() => {}}
        onRun={() => {}}
        onNew={() => {}}
        onEdit={() => {}}
        onExpandRun={() => {}}
        onRetryStep={() => {}}
        onCopyCommand={() => {}}
        onViewHistoryItem={() => {}}
      />,
    );
    await expectClean(workflows.container);
    workflows.unmount();

    const memory = render(
      <MemoryDrawer isOpen items={[memoryItem]} onClose={() => {}} onDelete={() => {}} />,
    );
    await expectClean(memory.container);
    memory.unmount();

    const settings = render(
      <SettingsDrawer
        isOpen
        onClose={() => {}}
        productMode="classic"
        onProductModeChange={() => {}}
        defaultInputMode="ask"
        onDefaultInputModeChange={() => {}}
        fontSize="md"
        onFontSizeChange={() => {}}
        simplifiedSummaries={false}
        onSimplifiedSummariesChange={() => {}}
        plannerModel="qwen2.5:14b"
        onPlannerModelChange={() => {}}
        plannerEndpoint="http://localhost:11434"
        onPlannerEndpointChange={() => {}}
        plannerStatus={null}
        onCheckPlanner={() => {}}
      />,
    );
    await expectClean(settings.container);
    settings.unmount();

    const palette = render(
      <CommandPalette
        isOpen
        onClose={() => {}}
        actions={[{ id: "new", label: "New Session", action: () => {} }]}
      />,
    );
    await expectClean(palette.container);
    palette.unmount();

    const editor = render(
      <WorkflowEditor initialLabel="Ship" initialSteps={["echo hi"]} onConfirm={() => {}} onCancel={() => {}} />,
    );
    await expectClean(editor.container);
    editor.unmount();

    const output = render(
      <OutputView
        blocks={[{ id: "e1", command: "echo hello", headline: "Finished. 1 line of output.", output: "hello" }]}
        onClose={() => {}}
      />,
    );
    await expectClean(output.container);
    output.unmount();

    const confirm = render(
      <ConfirmDialog title="Delete this workflow?" message="You can bring it back." confirmLabel="Delete workflow" onConfirm={() => {}} onCancel={() => {}} />,
    );
    await expectClean(confirm.container);
    confirm.unmount();

    const emptyHistory = render(
      <HistoryDrawer
        isOpen
        items={[]}
        allItems={[]}
        sessions={[session]}
        activeSessionId="s1"
        onClose={() => {}}
        onRerun={() => {}}
        onReopenPlan={() => {}}
        onSaveWorkflow={() => {}}
        onSaveSelected={() => {}}
        onCopyCommand={() => {}}
      />,
    );
    await expectClean(emptyHistory.container);
    emptyHistory.unmount();

    const emptyWorkflows = render(
      <WorkflowDrawer
        isOpen
        workflows={[]}
        lastRunByWorkflowId={{}}
        expandedRunWorkflowId={null}
        onClose={() => {}}
        onRun={() => {}}
        onNew={() => {}}
        onEdit={() => {}}
        onExpandRun={() => {}}
        onRetryStep={() => {}}
        onCopyCommand={() => {}}
        onViewHistoryItem={() => {}}
      />,
    );
    await expectClean(emptyWorkflows.container);
    emptyWorkflows.unmount();

    const emptyOutput = render(<OutputView blocks={[]} onClose={() => {}} />);
    await expectClean(emptyOutput.container);
    emptyOutput.unmount();

    const help = render(<HelpDialog onClose={() => {}} />);
    await expectClean(help.container);
    help.unmount();

    const plan = render(
      <PlanPanel
        sessionId="s1"
        intent="remove the notes folder"
        command="rm -rf notes"
        risk="high"
        explanation="Deletes the notes folder."
        flags={{ destructive: true }}
        safetyFlags={["DESTRUCTIVE_OPERATION"]}
        target={{ label: "Notes", cwd: "notes" }}
        onApprove={() => {}}
        onReject={() => {}}
        onSaveWorkflow={() => {}}
      />,
    );
    await expectClean(plan.container);
    plan.unmount();

    useWorkflowRunStore.setState({
      activeRun: {
        id: "run1",
        workflowId: "wf1",
        workflowName: "Ship the notes",
        startedAt: 0,
        status: "running",
        currentStepIndex: 0,
        steps: [
          { index: 0, command: "echo hi", status: "running" },
          { index: 1, command: "echo there", status: "pending" },
        ],
      },
      lastRunByWorkflowId: {},
    });
    const banner = render(<WorkflowRunBanner />);
    await expectClean(banner.container);
    banner.unmount();
  });

  it("names session tabs and keeps only one dialog open", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    const label = useSessionStore.getState().sessions[0]?.label ?? "";
    expect(screen.getByRole("tab", { selected: true })).toHaveTextContent(label);
    expect(screen.getByRole("button", { name: `Close ${label}` })).toBeInTheDocument();

    const history = screen.getByRole("button", { name: "History" });
    history.focus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("dialog", { name: "History" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("dialog", { name: "History" })).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens keyboard help with F1 and the output view with its button", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    fireEvent.keyDown(window, { key: "F1" });
    const help = await screen.findByRole("dialog", { name: "Keyboard and reading help" });
    expect(help).toHaveTextContent("Ctrl+J");
    expect(help).toHaveTextContent("Ctrl+Shift+R");
    expect(help).toHaveTextContent("Output view");
    await user.keyboard("{Escape}");

    await user.click(screen.getByRole("button", { name: "Output" }));
    expect(screen.getByRole("dialog", { name: "Output" })).toHaveTextContent(
      "No commands in this session yet. Run one, and its output will be listed here.",
    );
  });

  it("runs a command from the keyboard and announces the result once", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    const box = screen.getByRole("textbox", { name: "Command" });
    box.focus();
    fireEvent.keyDown(window, { key: "A", ctrlKey: true, shiftKey: true });
    expect(screen.getByRole("button", { name: "Command" })).toHaveAttribute("aria-pressed", "true");
    await user.type(box, "echo hello{Enter}");
    expect(await screen.findByText(/Finished\./)).toBeInTheDocument();
    expect(screen.getByTestId("result-line").querySelector("[role='status']")).toBeNull();
    await waitFor(() => {
      expect(document.querySelector("[role='status']")).toHaveTextContent(/Finished\./);
    });
  });

  it("changes the text size from the keyboard", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const slider = screen.getByRole("slider", { name: "Text size" });
    expect(slider).toHaveValue("100");
    fireEvent.change(slider, { target: { value: "200" } });
    expect(slider).toHaveValue("200");
    expect(document.querySelector(".app-shell")).toHaveStyle({ "--ui-scale": "2" });
    expect(screen.getByText("200%")).toBeInTheDocument();
  });

  it("asks, reviews, and hears the plan from the keyboard", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    const box = screen.getByRole("textbox", { name: "Command" });
    box.focus();
    await user.type(box, "list the files{Enter}");
    expect(await screen.findByRole("button", { name: "Reject" })).toBeInTheDocument();
    await waitFor(() => {
      expect(document.activeElement).toHaveClass("plan-panel");
      expect(document.querySelector("[role='status']")).toHaveTextContent(/A plan is ready to review/);
    });
  });

  it("moves focus from the command box to the result line", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    const box = screen.getByRole("textbox", { name: "Command" });
    box.focus();
    fireEvent.keyDown(window, { key: "A", ctrlKey: true, shiftKey: true });
    await user.type(box, "echo hello{Enter}");
    expect(await screen.findByText(/Finished\./)).toBeInTheDocument();
    box.focus();
    fireEvent.keyDown(window, { key: "R", ctrlKey: true, shiftKey: true });
    expect(document.activeElement).toBe(screen.getByTestId("result-line"));
  });

  it("shows Ask line breaks in the activity log", async () => {
    await renderReadyShell();
    const box = screen.getByRole("textbox", { name: "Command" });
    fireEvent.change(box, { target: { value: "list the files\nin this folder" } });
    await waitFor(() => expect(box).toHaveValue("list the files\nin this folder"));
    fireEvent.keyDown(box, { key: "Enter" });
    expect(await screen.findByRole("button", { name: "Reject" })).toBeInTheDocument();
    const log = screen.getByRole("log", { name: "CommandUI activity" });
    const ask = Array.from(log.querySelectorAll(".app-note")).find((note) =>
      note.textContent?.includes("list the files"),
    );
    expect(ask?.textContent).toContain("in this folder");
    expect(ask?.textContent).toContain("\n");
    expect(ask?.textContent).not.toContain("\\n");
    expect(log).toHaveAttribute("tabindex", "0");
  });
});
