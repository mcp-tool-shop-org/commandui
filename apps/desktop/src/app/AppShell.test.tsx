import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MemorySuggestion } from "@commandui/domain";
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
import { mockInvoke, resetMockBridge } from "../lib/mockBridge";
import { AppShell } from "./AppShell";

const xterm = vi.hoisted(() => ({
  writes: [] as string[],
  onData: null as null | ((data: string) => void),
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown> = {};
    textarea = document.createElement("textarea");
    loadAddon() {}
    open() {}
    write(data: string, callback?: () => void) {
      xterm.writes.push(data);
      callback?.();
    }
    clear() {}
    reset() {}
    focus() {}
    dispose() {}
    onData(handler: (data: string) => void) {
      xterm.onData = handler;
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

async function boot() {
  const user = userEvent.setup({ delay: null });
  render(<AppShell />);
  expect(await screen.findByText(/Welcome to CommandUI — Session 1/)).toBeInTheDocument();
  await waitFor(() => {
    expect(screen.getByPlaceholderText("Describe what you want to do…")).toBeEnabled();
  });
  return user;
}

describe("AppShell on the mock bridge", () => {
  beforeEach(() => {
    xterm.writes = [];
    xterm.onData = null;
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
    vi.restoreAllMocks();
  });

  it("boots a guided session in browser preview", async () => {
    await boot();
    expect(screen.getByText(/Browser preview mode/)).toBeInTheDocument();
    expect(screen.getByText(/v1\.0\.2/)).toBeInTheDocument();
    expect(screen.getByText("No semantic plan yet.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Session 1" })).toBeInTheDocument();
  });

  it("runs a typed command and keeps it in history", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    await user.type(screen.getByPlaceholderText("Submit a command explicitly…"), "echo hello{Enter}");
    await waitFor(() => {
      expect(xterm.writes.join("")).toContain("$ echo hello");
      expect(xterm.writes.join("")).toContain("hello");
    });

    await user.click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByText(/echo hello/)).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByPlaceholderText("Search history…")).toBeNull();
    });
  });

  it("refuses a command that is more than one line", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    const box = screen.getByPlaceholderText("Submit a command explicitly…");
    await user.type(box, "echo one{Shift>}{Enter}{/Shift}echo two{Enter}");
    expect(await screen.findByText(/more than one line/)).toBeInTheDocument();
    expect(box).toHaveValue("echo one\necho two");
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(/more than one line/)).toBeNull();
  });

  it("asks for a plan, rejects it, then approves the next one", async () => {
    const user = await boot();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "list the files{Enter}");
    expect(await screen.findByDisplayValue('echo "mock plan for: list the files"')).toBeInTheDocument();
    expect(screen.getByText('[plan] echo "mock plan for: list the files"')).toBeInTheDocument();
    expect(screen.getByText("? list the files")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reject" }));
    expect(screen.getByText("[rejected]")).toBeInTheDocument();
    expect(screen.getByText("No semantic plan yet.")).toBeInTheDocument();

    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "say hello{Enter}");
    expect(await screen.findByDisplayValue(/mock plan for: say hello/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Run Plan" }));
    expect(await screen.findByText(/\[approved] echo "mock plan for: say hello"/)).toBeInTheDocument();
    await waitFor(() => {
      expect(xterm.writes.join("")).toContain("mock plan for: say hello");
    });
  });

  it("saves a plan as a workflow and runs it", async () => {
    const user = await boot();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "ship it{Enter}");
    await screen.findByRole("button", { name: "Save Workflow" });
    await user.click(screen.getByRole("button", { name: "Save Workflow" }));
    expect(await screen.findByText("[workflow:saved] ship it")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Workflows" }));
    const drawer = document.querySelector(".settings-drawer");
    expect(drawer).not.toBeNull();
    expect(within(drawer as HTMLElement).getAllByText("ship it").length).toBeGreaterThan(0);
    await user.click(within(drawer as HTMLElement).getByRole("button", { name: "Run" }));
    expect(await screen.findByText(/\[workflow:done] ship it/, {}, { timeout: 5000 })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "History" }));
    // The saved plan and the workflow step both say "ship it". The step row is the one badged WF.
    await user.click(await screen.findByRole("button", { name: /ship it[\s\S]*WF/ }));
    await user.click(screen.getByRole("button", { name: "View workflow run" }));
    const workflows = document.querySelector(".settings-drawer") as HTMLElement;
    expect(workflows).not.toBeNull();
    expect(within(workflows).getByText("Workflows")).toBeInTheDocument();
    expect(within(workflows).getByText(/Started:/)).toBeInTheDocument();
  });

  it("opens another session, switches back, and closes one", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "New session" }));
    expect(await screen.findByText("[session] Session 2")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Session 1" }));
    expect(screen.getByText(/Welcome to CommandUI — Session 1/)).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "Close session" })[0]);
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Session 1" })).toBeNull();
    });
  });

  it("classic mode hides the empty plan, and a shortcut opens history", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const [mode] = screen.getAllByRole("combobox");
    await user.selectOptions(mode, "classic");
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByText("No semantic plan yet.")).toBeNull();

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "H", ctrlKey: true, shiftKey: true, bubbles: true }));
    expect(await screen.findByPlaceholderText("Search history…")).toBeInTheDocument();

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "K", ctrlKey: true, shiftKey: true, bubbles: true }),
    );
    expect(await screen.findByPlaceholderText("Type a command…")).toBeInTheDocument();
    await user.click(screen.getByText("Clear Terminal"));
    expect(screen.queryByText(/Welcome to CommandUI/)).toBeNull();
  });

  it("stops a running command and forwards a keystroke to the shell", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    await user.type(screen.getByPlaceholderText("Submit a command explicitly…"), "echo stay{Enter}");
    const stop = await screen.findByRole("button", { name: "Stop" });
    await user.click(stop);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Run" })).toBeEnabled();
    });
    xterm.onData?.("x");
    await waitFor(() => {
      expect(xterm.writes.join("")).toContain("echo stay");
    });
  });

  it("accepts a workflow pattern through the editor and dismisses another suggestion", async () => {
    const user = await boot();
    const pattern: MemorySuggestion = {
      id: "pat",
      scope: "project",
      projectRoot: "~/projects",
      kind: "workflow_pattern",
      label: "status then diff",
      proposedKey: "status then diff",
      proposedValue: JSON.stringify(["git status", "git diff"]),
      confidence: 0.8,
      derivedFromHistoryIds: ["h1"],
      status: "pending",
      createdAt: "2026-10-04T00:00:00Z",
    };
    const other: MemorySuggestion = {
      ...pattern,
      id: "other",
      kind: "recurring_command",
      label: "You frequently run git status",
      proposedKey: "git status",
      proposedValue: "git status",
    };
    mockInvoke("memory_store_suggestion", { request: { suggestion: pattern } });
    mockInvoke("memory_store_suggestion", { request: { suggestion: other } });
    useMemoryStore.getState().setMemorySuggestions([pattern, other]);

    expect(await screen.findByText("Workflow pattern")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "Accept" })[0]);
    expect(screen.getByRole("heading", { name: "Edit Workflow" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create Workflow" }));
    expect(await screen.findByText("[workflow:promoted] status then diff")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => {
      expect(screen.queryByText("You frequently run git status")).toBeNull();
    });
  });

  it("shows a background save failure and clears the terminal note", async () => {
    const user = await boot();
    window.dispatchEvent(
      new CustomEvent("commandui:persist-failed", {
        detail: { what: "history append", message: "disk full" },
      }),
    );
    expect(await screen.findByText(/Background save failed \(history append\): disk full/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.queryByText(/Welcome to CommandUI/)).toBeNull();
  });

  it("opens memory and deletes nothing when memory is empty", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Memory" }));
    expect(screen.getByText("No saved memory yet.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByText("No saved memory yet.")).toBeNull();
  });

  it("reruns a typed command from history", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    await user.type(screen.getByPlaceholderText("Submit a command explicitly…"), "echo hello{Enter}");
    await screen.findByRole("button", { name: "Stop" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());

    await user.click(screen.getByRole("button", { name: "History" }));
    await user.click(await screen.findByRole("button", { name: /echo hello/ }));
    await user.click(screen.getByRole("button", { name: "Rerun" }));
    await waitFor(() => {
      const prompts = xterm.writes.join("").split("$ echo hello").length - 1;
      expect(prompts).toBeGreaterThanOrEqual(2);
    });
    expect(screen.queryByPlaceholderText("Search history…")).toBeNull();
  });

  it("reopens a plan from history and runs it only after the risk check", async () => {
    const user = await boot();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "list the files{Enter}");
    await user.click(await screen.findByRole("button", { name: "Run Plan" }));
    expect(await screen.findByText(/\[approved]/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "History" }));
    await user.click(await screen.findByRole("button", { name: /list the files/ }));
    await user.click(screen.getByRole("button", { name: "View Plan" }));
    expect(await screen.findByText(/Reopened from history without a stored risk/)).toBeInTheDocument();
    const run = screen.getByRole("button", { name: "Run Plan" });
    expect(run).toBeDisabled();
    await user.click(screen.getByRole("checkbox", { name: /I understand the risks of this high-risk command/ }));
    expect(run).toBeEnabled();
    await user.click(run);
    expect(await screen.findByText(/\[approved] echo "mock plan for: list the files"/)).toBeInTheDocument();
  });

  it("saves a history row as a workflow and then deletes that workflow", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    await user.type(screen.getByPlaceholderText("Submit a command explicitly…"), "echo hello{Enter}");
    await screen.findByRole("button", { name: "Stop" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());

    await user.click(screen.getByRole("button", { name: "History" }));
    await user.click(await screen.findByRole("button", { name: /echo hello/ }));
    await user.click(screen.getByRole("button", { name: "Save Workflow" }));
    expect(await screen.findByText("[workflow:saved] echo hello")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Workflows" }));
    const drawer = document.querySelector(".settings-drawer") as HTMLElement;
    expect(within(drawer).getAllByText("echo hello").length).toBeGreaterThan(0);
    await user.click(within(drawer).getByRole("button", { name: "Delete" }));
    await waitFor(() => {
      expect(within(drawer).queryAllByText("echo hello")).toEqual([]);
    });
  });

  it("remembers an edited plan command as a substitution", async () => {
    const user = await boot();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "say hello{Enter}");
    const draft = await screen.findByDisplayValue(/mock plan for: say hello/);
    await user.clear(draft);
    await user.type(draft, "echo edited");
    await user.click(screen.getByRole("button", { name: "Run Plan" }));
    expect(await screen.findByText(/\[approved] echo edited/)).toBeInTheDocument();
    expect(await screen.findByText(/Use "echo edited" instead of/)).toBeInTheDocument();
  });

  it("offers a suggestion after the same command succeeds four times", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    const box = screen.getByPlaceholderText("Submit a command explicitly…");
    for (let n = 0; n < 4; n += 1) {
      await user.type(box, "echo hello{Enter}");
      await screen.findByRole("button", { name: "Stop" });
      await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());
    }
    expect(await screen.findByText(/You frequently run 'echo/)).toBeInTheDocument();
  });

  it("accepts a broken workflow pattern as memory and says why", async () => {
    const user = await boot();
    const pattern: MemorySuggestion = {
      id: "bad",
      scope: "project",
      projectRoot: "~/projects",
      kind: "workflow_pattern",
      label: "not really steps",
      proposedKey: "not really steps",
      proposedValue: "not-json",
      confidence: 0.5,
      derivedFromHistoryIds: [],
      status: "pending",
      createdAt: "2026-10-04T00:00:00Z",
    };
    mockInvoke("memory_store_suggestion", { request: { suggestion: pattern } });
    useMemoryStore.getState().setMemorySuggestions([pattern]);

    await user.click(await screen.findByRole("button", { name: "Accept" }));
    expect(await screen.findByText(/Could not read that workflow pattern/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Memory" }));
    expect(document.querySelector(".settings-drawer")?.textContent).toMatch(/not really steps/);
  });

  it("deletes one saved memory item", async () => {
    const user = await boot();
    const added = mockInvoke<{ item: { id: string; key: string; value: string } }>("memory_add", {
      request: { key: "workspace", value: "/work/app", kind: "preferred_cwd", scope: "project" },
    });
    useMemoryStore.getState().addMemoryItem(added.item as never);
    await user.click(screen.getByRole("button", { name: "Memory" }));
    const drawer = document.querySelector(".settings-drawer") as HTMLElement;
    expect(drawer.textContent).toMatch(/workspace/);
    await user.click(within(drawer).getAllByRole("button", { name: "Delete" })[0]);
    await waitFor(() => expect(screen.getByText("No saved memory yet.")).toBeInTheDocument());
  });

  it("moves a plan onto the session being viewed and files the run there", async () => {
    const user = await boot();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "list the files{Enter}");
    expect(await screen.findByRole("button", { name: "Run Plan" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "New session" }));
    expect(await screen.findByText(/you are viewing another session/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Run in the current session instead" }));
    await waitFor(() => {
      expect(screen.queryByText(/you are viewing another session/)).toBeNull();
    });
    // A new session stays "Terminal starting…" until its ready event. Run Plan before that
    // is refused and never files a history row.
    const composer = await screen.findByPlaceholderText("Describe what you want to do…");
    await waitFor(() => {
      expect(composer).toBeEnabled();
    });
    await user.click(screen.getByRole("button", { name: "Run Plan" }));
    expect(await screen.findByText(/\[approved] echo "mock plan for: list the files"/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Session 2" }).closest(".session-tab")).toHaveClass("active");
    await user.click(screen.getByRole("button", { name: "History" }));
    const history = document.querySelector(".history-drawer") as HTMLElement;
    expect(within(history).getByRole("button", { name: /list the files/ })).toBeInTheDocument();
  });

  it("closes a session that still has a command running", async () => {
    const user = await boot();
    await user.click(screen.getByRole("button", { name: "Command" }));
    await user.type(screen.getByPlaceholderText("Submit a command explicitly…"), "echo hello{Enter}");
    await screen.findByRole("button", { name: "Stop" });
    await user.click(screen.getAllByRole("button", { name: "Close session" })[0]);
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Session 1" })).toBeNull();
    });
    await user.click(screen.getByRole("button", { name: "History" }));
    const history = document.querySelector(".history-drawer") as HTMLElement;
    expect(within(history).getByRole("button", { name: /echo hello/ })).toHaveTextContent("interrupted");
  });

  it("interrupts a workflow step and says where it stopped", async () => {
    const user = await boot();
    await user.type(screen.getByPlaceholderText("Describe what you want to do…"), "ship it{Enter}");
    await user.click(await screen.findByRole("button", { name: "Save Workflow" }));
    expect(await screen.findByText("[workflow:saved] ship it")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Workflows" }));
    const drawer = document.querySelector(".settings-drawer") as HTMLElement;
    await user.click(within(drawer).getByRole("button", { name: "Run" }));
    await user.click(await screen.findByRole("button", { name: "Stop" }));
    expect(await screen.findByText(/\[workflow:interrupted] ship it/)).toBeInTheDocument();
  });
});
