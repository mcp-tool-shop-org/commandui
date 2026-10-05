import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MemorySuggestion } from "@commandui/domain";
import { MemorySuggestions } from "./MemorySuggestions";

function suggestion(overrides: Partial<MemorySuggestion> = {}): MemorySuggestion {
  return {
    id: "sug-1",
    scope: "global",
    kind: "recurring_command",
    label: "Get-ChildItem",
    proposedKey: "list",
    proposedValue: "Get-ChildItem",
    confidence: 0.6,
    derivedFromHistoryIds: ["h1", "h2", "h3", "h4"],
    status: "pending",
    createdAt: "2026-10-04T00:00:00.000Z",
    ...overrides,
  };
}

describe("MemorySuggestions", () => {
  it("explains the count and the percentage in words", () => {
    render(
      <MemorySuggestions
        suggestions={[
          suggestion(),
          suggestion({
            id: "sug-2",
            kind: "preferred_cwd",
            label: "C:\\Work\\demo",
            confidence: 0.6,
          }),
        ]}
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getAllByText("Seen in 4 commands you ran.").length).toBeGreaterThan(0);
    expect(screen.getAllByText("CommandUI is 60% sure.").length).toBeGreaterThan(0);
    expect(screen.getByText("Frequent command")).toBeInTheDocument();
    expect(screen.getByText("Preferred workspace")).toBeInTheDocument();
    expect(screen.queryByText("FREQUENT COMMAND")).toBeNull();
    expect(screen.queryByText("PREFERRED WORKSPACE")).toBeNull();
    expect(screen.queryByText(/^60%$/)).toBeNull();
    const panel = screen.getByRole("region", { name: "Memory suggestions" });
    expect(panel).toHaveAttribute("tabindex", "0");
  });

  it("hides the suggestions and shows them again", async () => {
    const user = userEvent.setup();
    render(
      <MemorySuggestions
        suggestions={[suggestion()]}
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Hide memory suggestions" }));
    expect(screen.queryByRole("region", { name: "Memory suggestions" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Accept" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show memory suggestions" }));
    expect(screen.getByRole("region", { name: "Memory suggestions" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Accept" })).toBeVisible();
  });

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
          suggestion({ id: "keep", status: "dismissed", label: "You frequently run 'git status'" }),
          suggestion({ id: "show", kind: "made_up" as MemorySuggestion["kind"], derivedFromHistoryIds: [], confidence: 0.5, label: "Odd one" }),
        ]}
      />,
    );

    expect(screen.queryByText("You frequently run 'git status'")).toBeNull();
    expect(screen.getByText("made_up")).toBeInTheDocument();
    expect(screen.getByText("Odd one")).toBeInTheDocument();
    expect(screen.getByText("CommandUI is 50% sure.")).toBeInTheDocument();
    expect(screen.queryByText(/Based on/)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Accept" }));
    expect(onAccept).toHaveBeenCalledWith("show");
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledWith("show");
  });

  it("names a known kind and counts the commands it came from", () => {
    render(
      <MemorySuggestions
        suggestions={[suggestion({ id: "s", label: "You frequently run 'git status'", derivedFromHistoryIds: ["h1", "h2"], confidence: 0.66 })]}
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getByText("Frequent command")).toBeInTheDocument();
    expect(screen.getByText("Seen in 2 commands you ran.")).toBeInTheDocument();
    expect(screen.getByText("CommandUI is 66% sure.")).toBeInTheDocument();
  });
});
