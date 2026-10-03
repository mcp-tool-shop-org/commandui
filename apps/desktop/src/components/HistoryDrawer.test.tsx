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
