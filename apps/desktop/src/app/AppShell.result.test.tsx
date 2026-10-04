import { cleanup, render, screen, waitFor } from "@testing-library/react";
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
import { resetMockBridge } from "../lib/mockBridge";
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
  render(<AppShell />);
  await waitFor(() => {
    expect(screen.getByPlaceholderText("Describe what you want to do…")).toBeEnabled();
  });
}

describe("Workstream 1 result line", () => {
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

  it("offers to Ask instead of running a sentence typed in Command mode", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    await user.click(screen.getByRole("button", { name: /^Command$/ }));
    const box = await screen.findByPlaceholderText("Submit a command explicitly…");
    await user.type(box, "list the files");
    await user.click(screen.getByRole("button", { name: /^Run$/ }));

    expect(await screen.findByText(/This looks like a request\. Ask CommandUI instead\?/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ask CommandUI instead" })).toBeInTheDocument();
    expect(useHistoryStore.getState().items.some((item) => item.userInput === "list the files")).toBe(false);

    await user.click(screen.getByRole("button", { name: "Ask CommandUI instead" }));
    expect(screen.getByPlaceholderText("Describe what you want to do…")).toHaveValue("list the files");
    expect(useHistoryStore.getState().items.some((item) => item.userInput === "list the files")).toBe(false);
  });

  it("still runs a real command typed in Command mode", async () => {
    const user = userEvent.setup();
    await renderReadyShell();
    await user.click(screen.getByRole("button", { name: /^Command$/ }));
    const box = await screen.findByPlaceholderText("Submit a command explicitly…");
    await user.type(box, "echo hello");
    await user.click(screen.getByRole("button", { name: /^Run$/ }));

    expect(screen.queryByText(/This looks like a request/)).not.toBeInTheDocument();
    await waitFor(() => {
      expect(useHistoryStore.getState().items.some((item) => item.userInput === "echo hello")).toBe(true);
    });
    expect(await screen.findByText(/Finished\./)).toBeInTheDocument();
    expect(screen.queryByText(/^failure$/i)).not.toBeInTheDocument();
  });
});
