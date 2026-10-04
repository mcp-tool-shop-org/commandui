import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
import { mockInvoke, onMockEvent, resetMockBridge } from "../lib/mockBridge";
import { AppShell } from "./AppShell";

const runtime = vi.hoisted(() => ({ failBoot: false }));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown> = {};
    textarea = document.createElement("textarea");
    loadAddon() {}
    open() {}
    write(_data: string, callback?: () => void) {
      callback?.();
    }
    clear() {}
    reset() {}
    focus() {}
    dispose() {}
    onData() {
      return { dispose() {} };
    }
    attachCustomKeyEventHandler() {}
    replay() {}
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => {
    if (runtime.failBoot && (command === "session_list" || command === "session_create")) {
      return Promise.reject(new Error("backend down"));
    }
    return mockInvoke(command, args);
  },
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (emitted: { payload: unknown }) => void) => {
    const stop = onMockEvent(event, (payload: unknown) => handler({ payload }));
    return Promise.resolve(stop);
  },
}));

function resetStores() {
  useComposerStore.setState({ inputValue: "", inputMode: "command" });
  useExecutionStore.setState({
    activeExecutionId: null,
    lastExecutionId: null,
    executionStatus: "idle",
    sessionExecStates: {},
  });
  useHistoryStore.setState({ items: [] });
  useSessionStore.setState({ sessions: [], activeSessionId: null });
  useMemoryStore.setState({ items: [], suggestions: [] });
  useSettingsStore.setState({
    productMode: "classic",
    reducedClutter: false,
    simplifiedSummaries: false,
    confirmMediumRisk: true,
    defaultInputMode: "command",
  });
  useWorkflowStore.setState({ items: [] });
  useWorkflowRunStore.setState({ activeRun: null, lastRunByWorkflowId: {} });
  useFocusStore.setState({ currentZone: null, previousZone: null });
}

describe("AppShell against the Tauri bridge", () => {
  beforeEach(() => {
    runtime.failBoot = false;
    (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
    resetStores();
    resetMockBridge();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    cleanup();
    resetMockBridge();
    delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
    vi.restoreAllMocks();
  });

  it("shows a recovered session as out of sync, then resync makes it ready", async () => {
    const user = userEvent.setup({ delay: null });
    mockInvoke("session_create", { request: { label: "Recovered" } });
    await new Promise((resolve) => setTimeout(resolve, 150));

    render(<AppShell />);
    expect(screen.queryByText(/Browser preview mode/)).toBeNull();
    expect(await screen.findByText("Terminal appears desynced.")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Terminal out of sync — use Resync.")).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Resync" }));
    await waitFor(() => {
      expect(screen.queryByText("Terminal appears desynced.")).toBeNull();
    });
    await waitFor(() => {
      expect(screen.getByPlaceholderText("Describe what you want to do…")).toBeEnabled();
    });

    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.selectOptions(screen.getAllByRole("combobox")[0], "classic");
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByText("No semantic plan yet.")).toBeNull();

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "X", ctrlKey: true, shiftKey: true, bubbles: true }),
    );
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Recovered" })).toBeNull();
    });
  });

  it("says the app could not start when the backend is down", async () => {
    const user = userEvent.setup({ delay: null });
    runtime.failBoot = true;
    const navigationErrors: string[] = [];
    const virtualConsole = (window as unknown as {
      _virtualConsole?: { on: (event: string, fn: (error: Error) => void) => void };
    })._virtualConsole;
    virtualConsole?.on("jsdomError", (error) => navigationErrors.push(error.message));

    render(<AppShell />);
    expect(await screen.findByRole("heading", { name: "CommandUI could not start" })).toBeInTheDocument();
    expect(screen.getByText(/backend down/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(navigationErrors).toContain("Not implemented: navigation (except hash changes)");
  });
});
