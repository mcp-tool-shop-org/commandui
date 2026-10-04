import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MemorySuggestion } from "@commandui/domain";
import { MemorySuggestions } from "./MemorySuggestions";

function suggestion(partial: Partial<MemorySuggestion> & Pick<MemorySuggestion, "id">): MemorySuggestion {
  return {
    scope: "project",
    kind: "recurring_command",
    label: "You frequently run 'git status'",
    proposedKey: "git status",
    proposedValue: "git status",
    confidence: 0.66,
    derivedFromHistoryIds: ["h1", "h2"],
    status: "pending",
    createdAt: "2026-10-04T00:00:00Z",
    ...partial,
  };
}

describe("MemorySuggestions", () => {
  it("renders nothing when every suggestion is already settled", () => {
    const { container } = render(
      <MemorySuggestions
        suggestions={[suggestion({ id: "a", status: "accepted" })]}
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the pending suggestion and reports accept or dismiss", async () => {
    const user = userEvent.setup();
    const onAccept = vi.fn();
    const onDismiss = vi.fn();
    render(
      <MemorySuggestions
        onAccept={onAccept}
        onDismiss={onDismiss}
        suggestions={[
          suggestion({ id: "keep", status: "dismissed" }),
          suggestion({ id: "show", kind: "made_up" as MemorySuggestion["kind"], derivedFromHistoryIds: [], confidence: 0.5, label: "Odd one" }),
        ]}
      />,
    );

    expect(screen.queryByText("You frequently run 'git status'")).toBeNull();
    expect(screen.getByText("made_up")).toBeInTheDocument();
    expect(screen.getByText("Odd one")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(screen.queryByText(/Based on/)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Accept" }));
    expect(onAccept).toHaveBeenCalledWith("show");
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledWith("show");
  });

  it("names a known kind and counts the executions it came from", () => {
    render(
      <MemorySuggestions
        suggestions={[suggestion({ id: "s" })]}
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getByText("Frequent command")).toBeInTheDocument();
    expect(screen.getByText("Based on 2 executions")).toBeInTheDocument();
    expect(screen.getByText("66%")).toBeInTheDocument();
  });
});
