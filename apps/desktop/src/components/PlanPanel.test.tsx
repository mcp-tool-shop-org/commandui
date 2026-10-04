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

  it("requires the folder name before a high-risk plan can run", async () => {
    const onApprove = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        risk="high"
        target={{ label: "Session 1", cwd: "C:\\Work\\notes" }}
        onApprove={onApprove}
      />,
    );

    const runButton = screen.getByText("Run Plan");
    expect(runButton).toBeDisabled();
    const confirm = screen.getByLabelText("Type notes to run this");
    await userEvent.type(confirm, "nope");
    expect(runButton).toBeDisabled();
    await userEvent.clear(confirm);
    await userEvent.type(confirm, "notes");
    expect(runButton).not.toBeDisabled();
    await userEvent.click(runButton);
    expect(onApprove).toHaveBeenCalledWith("git status --short");
    expect(screen.getByText("Shows a short list of what changed in this folder.")).toBeInTheDocument();
  });

  it("approves the trimmed command for high risk", async () => {
    const onApprove = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        command={"  git push  "}
        risk="high"
        onApprove={onApprove}
      />,
    );
    await userEvent.type(screen.getByLabelText("Type confirm to run this"), "confirm");
    await userEvent.click(screen.getByText("Run Plan"));
    expect(onApprove).toHaveBeenCalledWith("git push");
  });

  it("approves a medium-risk plan in one step", async () => {
    const onApprove = vi.fn();
    render(<PlanPanel {...defaultProps} risk="medium" onApprove={onApprove} />);
    expect(screen.queryByLabelText(/to run this/)).toBeNull();
    const runButton = screen.getByText("Run Plan");
    expect(runButton).not.toBeDisabled();
    await userEvent.click(runButton);
    expect(onApprove).toHaveBeenCalledWith("git status --short");
  });

  it("writes safety flags in plain words and lists a delete", () => {
    render(
      <PlanPanel
        {...defaultProps}
        command="rm notes.txt old.log"
        risk="high"
        flags={{ destructive: true }}
        safetyFlags={["DESTRUCTIVE_OPERATION", "NETWORK_ACCESS"]}
      />,
    );
    expect(screen.getByText("Deletes files: cannot be undone")).toBeInTheDocument();
    expect(screen.getByText("Uses the network")).toBeInTheDocument();
    expect(screen.getByText("This will delete: notes.txt, old.log")).toBeInTheDocument();
    expect(screen.queryByText("DESTRUCTIVE_OPERATION")).toBeNull();
    expect(screen.queryByText("NETWORK_ACCESS")).toBeNull();
    expect(screen.getByText("Run Plan")).toBeDisabled();
  });

  it("reports the run gate with the trimmed edited command and confirmation", async () => {
    const onRunGate = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        command={"  git push  "}
        risk="high"
        onRunGate={onRunGate}
      />,
    );
    expect(onRunGate).toHaveBeenLastCalledWith({
      command: "git push",
      confirmed: false,
    });

    await userEvent.type(screen.getByLabelText("Type confirm to run this"), "confirm");
    expect(onRunGate).toHaveBeenLastCalledWith({
      command: "git push",
      confirmed: true,
    });

    const textarea = screen.getByLabelText("Command");
    await userEvent.clear(textarea);
    await userEvent.type(textarea, "  git fetch ");
    expect(onRunGate).toHaveBeenLastCalledWith({
      command: "git fetch",
      confirmed: true,
    });
  });

  it("reports an empty gate when there is no command", () => {
    const onRunGate = vi.fn();
    render(<PlanPanel {...defaultProps} command="" onRunGate={onRunGate} />);
    expect(onRunGate).toHaveBeenLastCalledWith({ command: "", confirmed: false });
  });

  it("does not call a file listing a change", () => {
    render(
      <PlanPanel
        {...defaultProps}
        command="Get-ChildItem -File -Filter *.log | Sort-Object Length -Descending | Select-Object -First 3"
        risk="low"
        flags={{ touchesFiles: true }}
        explanation=""
      />,
    );
    expect(screen.getByText("Low risk. Easy to undo.")).toBeInTheDocument();
    expect(screen.queryByText("Changes files")).toBeNull();
    expect(
      screen.getByText(
        "Lists files whose names match *.log, then sorts them by Length, largest first, then keeps the first 3.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("Passed to the program")).toBeNull();
    expect(screen.queryByText("Runs Get-ChildItem.")).toBeNull();
  });

  it("shows empty state when no command", () => {
    render(<PlanPanel {...defaultProps} command="" />);
    expect(screen.getByText(/no plan yet/i)).toBeDefined();
  });
});
