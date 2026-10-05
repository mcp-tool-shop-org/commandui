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
});
