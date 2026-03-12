import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PlanPanel } from "./PlanPanel";

describe("PlanPanel", () => {
  const defaultProps = {
    sessionId: "test-session",
    intent: "show changed files",
    command: "git status --short",
    risk: "low" as const,
    explanation: "Shows modified files",
    onApprove: vi.fn(),
    onReject: vi.fn(),
    onSaveWorkflow: vi.fn(),
  };

  it("calls onApprove with edited command", async () => {
    const onApprove = vi.fn();
    render(<PlanPanel {...defaultProps} onApprove={onApprove} />);

    const textarea = screen.getByDisplayValue("git status --short");
    await userEvent.clear(textarea);
    await userEvent.type(textarea, "git diff --stat");
    await userEvent.click(screen.getByText("Run Plan"));

    expect(onApprove).toHaveBeenCalledWith("git diff --stat");
  });

  it("requires confirmation for high risk", async () => {
    const onApprove = vi.fn();
    render(
      <PlanPanel {...defaultProps} risk="high" onApprove={onApprove} />,
    );

    // Run Plan should be disabled without checkbox
    const runButton = screen.getByText("Run Plan");
    expect(runButton).toBeDisabled();

    // Check the confirmation checkbox
    const checkbox = screen.getByRole("checkbox");
    await userEvent.click(checkbox);

    // Now Run Plan should be enabled
    expect(runButton).not.toBeDisabled();
    await userEvent.click(runButton);
    expect(onApprove).toHaveBeenCalled();
  });

  it("shows empty state when no command", () => {
    render(<PlanPanel {...defaultProps} command="" />);
    expect(screen.getByText(/no semantic plan yet/i)).toBeDefined();
  });
});
