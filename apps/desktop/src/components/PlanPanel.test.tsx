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
    expect(onApprove).toHaveBeenCalledWith("git status --short");
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
    await userEvent.click(screen.getByRole("checkbox"));
    await userEvent.click(screen.getByText("Run Plan"));
    expect(onApprove).toHaveBeenCalledWith("git push");
  });

  it("requires confirmation for medium risk by default", async () => {
    const onApprove = vi.fn();
    render(<PlanPanel {...defaultProps} risk="medium" onApprove={onApprove} />);
    const runButton = screen.getByText("Run Plan");
    expect(runButton).toBeDisabled();
    await userEvent.click(screen.getByRole("checkbox"));
    expect(runButton).not.toBeDisabled();
    await userEvent.click(runButton);
    expect(onApprove).toHaveBeenCalledWith("git status --short");
  });

  it("runs medium risk without confirmation when not required", async () => {
    const onApprove = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        risk="medium"
        requireMediumRiskConfirmation={false}
        onApprove={onApprove}
      />,
    );
    expect(screen.queryByRole("checkbox")).toBeNull();
    const runButton = screen.getByText("Run Plan");
    expect(runButton).not.toBeDisabled();
    await userEvent.click(runButton);
    expect(onApprove).toHaveBeenCalledWith("git status --short");
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

    await userEvent.click(screen.getByRole("checkbox"));
    expect(onRunGate).toHaveBeenLastCalledWith({
      command: "git push",
      confirmed: true,
    });

    const textarea = screen.getByRole("textbox");
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

  it("shows empty state when no command", () => {
    render(<PlanPanel {...defaultProps} command="" />);
    expect(screen.getByText(/no semantic plan yet/i)).toBeDefined();
  });

  it("rejects, saves, and shows where the plan runs", async () => {
    const onReject = vi.fn();
    const onSaveWorkflow = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        plannerSource="mock"
        target={{ label: "Session 1", cwd: "/work" }}
        contextSources={["cwd: /work", "workflow:Ship"]}
        onReject={onReject}
        onSaveWorkflow={onSaveWorkflow}
      />,
    );
    expect(screen.getByText("Mock planner — Ollama not connected")).toBeInTheDocument();
    expect(screen.getByText("Session 1")).toBeInTheDocument();
    expect(screen.getByText("Context: cwd: /work, workflow:Ship")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Save Workflow" }));
    expect(onSaveWorkflow).toHaveBeenCalledWith("git status --short");
    await userEvent.click(screen.getByRole("button", { name: "Reject" }));
    expect(onReject).toHaveBeenCalledOnce();
  });

  it("explains why a plan cannot run and offers both ways forward", async () => {
    const onGoToTarget = vi.fn();
    const onRetarget = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        blockedReason="The session this plan was made for is closed."
        notice="It will run in the open session."
        onGoToTarget={onGoToTarget}
        onRetarget={onRetarget}
        retargetLabel="Run here"
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("The session this plan was made for is closed.");
    expect(screen.getByText("It will run in the open session.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Plan" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Go to that session" }));
    await userEvent.click(screen.getByRole("button", { name: "Run here" }));
    expect(onGoToTarget).toHaveBeenCalledOnce();
    expect(onRetarget).toHaveBeenCalledOnce();
  });

  it("shows safety flags and blocks a command that hides characters", async () => {
    const onApprove = vi.fn();
    render(
      <PlanPanel
        {...defaultProps}
        command={"git status\u202e"}
        risk="low"
        flags={{ destructive: true, touchesFiles: true, touchesNetwork: true, escalatesPrivileges: true, requiresConfirmation: true }}
        safetyFlags={["deletes files"]}
        ambiguityFlags={["which repo?"]}
        onApprove={onApprove}
      />,
    );
    expect(screen.getByText("destructive")).toBeInTheDocument();
    expect(screen.getByText("touches files")).toBeInTheDocument();
    expect(screen.getByText("uses the network")).toBeInTheDocument();
    expect(screen.getByText("escalates privileges")).toBeInTheDocument();
    expect(screen.getByText(/deletes files/)).toBeInTheDocument();
    expect(screen.getByText(/which repo/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Plan" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save Workflow" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Remove hidden characters" }));
    await userEvent.click(screen.getByRole("checkbox"));
    await userEvent.click(screen.getByRole("button", { name: "Run Plan" }));
    expect(onApprove).toHaveBeenCalledWith("git status");
  });
});
