import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import appShellSource from "./AppShell.tsx?raw";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
import type { MemoryItem, Workflow } from "@commandui/domain";
import { ERROR_CODES } from "../lib/commandError";
import { resetMockBridge } from "../lib/mockBridge";
import { AppShell } from "./AppShell";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
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

const WORKFLOW: Workflow = {
  id: "wf-undo",
  label: "Ship the notes",
  source: "raw",
  command: "echo hi",
  createdAt: "2026-10-04T00:00:00.000Z",
};

const MEMORY: MemoryItem = {
  id: "mem-undo",
  scope: "global",
  kind: "tool_preference",
  key: "editor",
  value: "helix",
  confidence: 1,
  source: "manual",
  createdAt: "2026-10-04T00:00:00.000Z",
  updatedAt: "2026-10-04T00:00:00.000Z",
};

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
    confirmMediumRisk: true,
    defaultInputMode: "command",
  });
  useComposerStore.setState({ inputValue: "", inputMode: "command" });
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

describe("Workstream 0 honesty", () => {
  beforeEach(() => {
    resetStores();
    resetMockBridge();
    localStorage.clear();
    invoke.mockReset();
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    vi.spyOn(window, "confirm").mockImplementation(() => {
      throw new Error("native confirm should not be used");
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    resetMockBridge();
    resetStores();
    localStorage.clear();
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("keeps Escape from rejecting a plan, and R still rejects it", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "list the files");
    await user.click(screen.getByRole("button", { name: "Draft plan" }));

    await waitFor(() => {
      const item = useHistoryStore.getState().items.find((entry) => entry.userInput === "list the files");
      expect(item?.plannerSource).toBe("mock");
      expect(item?.status).toBe("planned");
    });
    expect(screen.getByRole("button", { name: "Reject" })).toBeInTheDocument();

    const panel = document.querySelector(".plan-panel");
    expect(panel).toBeTruthy();
    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByRole("button", { name: "Reject" })).toBeInTheDocument();
    expect(useHistoryStore.getState().items.find((entry) => entry.userInput === "list the files")?.status).toBe("planned");

    // React listens for focusin. element.focus() is what moves the zone to "plan",
    // which is the only place the R key is allowed to reject.
    (panel as HTMLElement).focus();
    await waitFor(() => {
      expect(useFocusStore.getState().currentZone).toBe("plan");
    });
    fireEvent.keyDown(window, { key: "r" });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Reject" })).not.toBeInTheDocument();
    });
    expect(useHistoryStore.getState().items.find((entry) => entry.userInput === "list the files")?.status).toBe("rejected");
  });

  it("deletes a workflow only after confirm, and Undo puts it back", async () => {
    await renderReadyShell();
    useWorkflowStore.getState().addWorkflow(WORKFLOW);
    fireEvent.click(screen.getByRole("button", { name: "Workflows" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(screen.getByRole("dialog", { name: "Delete this workflow?" })).toBeInTheDocument();
    expect(screen.getByText("Ship the notes")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByText("Ship the notes")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete workflow" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Delete this workflow?" })).not.toBeInTheDocument();
      expect(useWorkflowStore.getState().items.some((item) => item.id === "wf-undo")).toBe(false);
    });
    expect(screen.queryByText("Ship the notes")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(screen.getByText("Ship the notes")).toBeInTheDocument();
    });
    expect(useWorkflowStore.getState().items.some((item) => item.id === "wf-undo")).toBe(true);
  });

  it("commits a workflow delete after 10 seconds without Undo", async () => {
    await renderReadyShell();
    useWorkflowStore.getState().addWorkflow(WORKFLOW);
    fireEvent.click(screen.getByRole("button", { name: "Workflows" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(screen.getByRole("dialog", { name: "Delete this workflow?" })).toBeInTheDocument();

    vi.useFakeTimers();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Delete workflow" }));
    });
    expect(screen.queryByRole("dialog", { name: "Delete this workflow?" })).not.toBeInTheDocument();
    expect(useWorkflowStore.getState().items.some((item) => item.id === "wf-undo")).toBe(false);
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    expect(useWorkflowStore.getState().items.some((item) => item.id === "wf-undo")).toBe(false);
  });

  it("restores a deleted memory item on Undo", async () => {
    await renderReadyShell();
    useMemoryStore.getState().addMemoryItem(MEMORY);
    fireEvent.click(screen.getByRole("button", { name: "Memory" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(screen.getByRole("dialog", { name: "Delete this memory?" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Delete this memory?" })).not.toBeInTheDocument();
    });
    expect(useMemoryStore.getState().items.some((item) => item.id === "mem-undo")).toBe(true);
    expect(screen.getByText(/editor/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete memory" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Delete this memory?" })).not.toBeInTheDocument();
      expect(useMemoryStore.getState().items.some((item) => item.id === "mem-undo")).toBe(false);
    });
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(useMemoryStore.getState().items.some((item) => item.id === "mem-undo")).toBe(true);
    });
    expect(screen.getByText(/editor/)).toBeInTheDocument();
  });

  it.each(ERROR_CODES)("renders the Rust message for %s", async (code) => {
    (window as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
    invoke.mockImplementation(async (command: string) => {
      if (command === "settings_get") return { settings: {} };
      if (command === "session_list") return { sessions: [] };
      if (command === "session_create") {
        throw { code, message: `rust message for ${code}`, details: "more detail" };
      }
      return {};
    });
    render(<AppShell />);
    expect(await screen.findByText(`rust message for ${code} more detail`)).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("[object Object]");
    expect(document.body.textContent).not.toContain("Command '");
    expect(document.body.textContent).not.toMatch(/Tauri|backend|pnpm tauri:dev/);
  });

  it("does not ship the developer strings or the old matchers", () => {
    expect(appShellSource).not.toContain("window.confirm");
    expect(appShellSource).not.toContain("pnpm tauri:dev");
    expect(appShellSource).not.toContain("Tauri backend");
    expect(appShellSource).not.toContain("isMissingIdError");
    expect(appShellSource).not.toContain("/has exited/");
    expect(appShellSource).not.toContain('browserPreview ? "mock"');
  });
});
