import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { HistoryItem, SessionSummary } from "@commandui/domain";
import { HistoryDrawer } from "./HistoryDrawer";

const item = (id: string, sessionId: string): HistoryItem => ({
  id,
  sessionId,
  source: "raw",
  userInput: `input-${id}`,
  executedCommand: `cmd-${id}`,
  status: "success",
  createdAt: new Date().toISOString(),
});

const s1a = item("a1", "s1");
const s1b = item("a2", "s1");
const s2a = item("b1", "s2");
const s2b = item("b2", "s2");
const allItems = [s1a, s1b, s2a, s2b];

const sessions = [
  { id: "s1", label: "Session One" },
  { id: "s2", label: "Session Two" },
] as unknown as SessionSummary[];

function renderDrawer() {
  const noop = vi.fn();
  render(
    <HistoryDrawer
      isOpen
      items={[s1a, s1b]}
      allItems={allItems}
      sessions={sessions}
      activeSessionId="s1"
      onClose={noop}
      onRerun={noop}
      onReopenPlan={noop}
      onSaveWorkflow={noop}
      onCopyCommand={noop}
    />,
  );
  return screen.getByRole("combobox");
}

const shown = () =>
  allItems.filter((i) => screen.queryByText(i.userInput, { exact: false })).map((i) => i.id);

describe("HistoryDrawer session filter", () => {
  it("defaults to the current session items", () => {
    renderDrawer();
    expect(shown()).toEqual(["a1", "a2"]);
  });

  it("'all' shows every session", () => {
    const select = renderDrawer();
    fireEvent.change(select, { target: { value: "all" } });
    expect(shown()).toEqual(["a1", "a2", "b1", "b2"]);
  });

  it("a specific session id shows only that session, including rows absent from items", () => {
    const select = renderDrawer();
    fireEvent.change(select, { target: { value: "s2" } });
    expect(shown()).toEqual(["b1", "b2"]);

    fireEvent.change(select, { target: { value: "s1" } });
    expect(shown()).toEqual(["a1", "a2"]);

    fireEvent.change(select, { target: { value: "current" } });
    expect(shown()).toEqual(["a1", "a2"]);
  });
});

describe("HistoryDrawer rows", () => {
  it("renders nothing while closed and skeletons while loading", () => {
    const { rerender } = render(
      <HistoryDrawer
        isOpen={false}
        items={[]}
        allItems={[]}
        sessions={sessions}
        activeSessionId="s1"
        onClose={vi.fn()}
        onRerun={vi.fn()}
        onReopenPlan={vi.fn()}
        onSaveWorkflow={vi.fn()}
        onCopyCommand={vi.fn()}
      />,
    );
    expect(screen.queryByText("History")).toBeNull();
    rerender(
      <HistoryDrawer
        isOpen
        loading
        items={allItems}
        allItems={allItems}
        sessions={sessions}
        activeSessionId="s1"
        onClose={vi.fn()}
        onRerun={vi.fn()}
        onReopenPlan={vi.fn()}
        onSaveWorkflow={vi.fn()}
        onCopyCommand={vi.fn()}
      />,
    );
    expect(document.querySelectorAll(".skeleton-item")).toHaveLength(3);
  });

  it("says when the filter matches nothing, and searches the visible rows", () => {
    render(
      <HistoryDrawer
        isOpen
        items={[]}
        allItems={[]}
        sessions={sessions}
        activeSessionId="s1"
        onClose={vi.fn()}
        onRerun={vi.fn()}
        onReopenPlan={vi.fn()}
        onSaveWorkflow={vi.fn()}
        onCopyCommand={vi.fn()}
      />,
    );
    expect(screen.getByText("No history yet.")).toBeInTheDocument();
  });

  it("searches, expands, and runs the row actions", () => {
    const onRerun = vi.fn();
    const onReopenPlan = vi.fn();
    const onSaveWorkflow = vi.fn();
    const onCopyCommand = vi.fn();
    const onViewWorkflowRun = vi.fn();
    const onClose = vi.fn();
    const semantic: HistoryItem = {
      id: "sem",
      sessionId: "s1",
      source: "semantic",
      userInput: "list the files",
      generatedCommand: "ls",
      executedCommand: "ls",
      status: "success",
      createdAt: new Date(Date.now() - 120_000).toISOString(),
      durationMs: 1_500,
      exitCode: 0,
      cwd: "/work",
      plannerSource: "mock",
      workflowRunId: "run-1",
    };
    const rejected: HistoryItem = {
      ...semantic,
      id: "nope",
      userInput: "do not run",
      status: "rejected",
      workflowRunId: undefined,
      durationMs: 12,
    };
    render(
      <HistoryDrawer
        isOpen
        items={[semantic, rejected]}
        allItems={[semantic, rejected]}
        sessions={sessions}
        activeSessionId="s1"
        onClose={onClose}
        onRerun={onRerun}
        onReopenPlan={onReopenPlan}
        onSaveWorkflow={onSaveWorkflow}
        onCopyCommand={onCopyCommand}
        onViewWorkflowRun={onViewWorkflowRun}
        initialExpandedId="sem"
      />,
    );

    fireEvent.change(screen.getByPlaceholderText("Search history…"), { target: { value: "do not" } });
    expect(screen.queryByText(/list the files/)).toBeNull();
    expect(screen.getByText(/do not run/)).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("Search history…"), { target: { value: "" } });
    const row = screen.getByRole("button", { name: /list the files/ });
    expect(row).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByText("semantic/mock").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("1.5s").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("12ms")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Rerun" }));
    expect(onRerun).toHaveBeenCalledWith(expect.objectContaining({ id: "sem" }));
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(onCopyCommand).toHaveBeenCalledWith("ls");
    fireEvent.click(screen.getByRole("button", { name: "View Plan" }));
    expect(onReopenPlan).toHaveBeenCalledWith(expect.objectContaining({ id: "sem" }));
    fireEvent.click(screen.getByRole("button", { name: "Save Workflow" }));
    expect(onSaveWorkflow).toHaveBeenCalledWith(expect.objectContaining({ id: "sem" }));
    fireEvent.click(screen.getByRole("button", { name: "View workflow run" }));
    expect(onViewWorkflowRun).toHaveBeenCalledWith("run-1");
    fireEvent.click(screen.getByText("WF"));
    expect(onViewWorkflowRun).toHaveBeenCalledTimes(2);

    const rejectedRow = screen.getByRole("button", { name: /do not run/ });
    fireEvent.keyDown(rejectedRow, { key: "Enter" });
    expect(rejectedRow).toHaveAttribute("aria-expanded", "true");
    const reruns = screen.getAllByRole("button", { name: "Rerun" });
    expect(reruns[reruns.length - 1]).toBeDisabled();
    fireEvent.keyDown(rejectedRow, { key: " " });
    expect(rejectedRow).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(screen.getByText("History"));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(document.querySelector(".history-overlay")!);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
