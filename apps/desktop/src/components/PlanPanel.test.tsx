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

  it("describes the folders it looked at without field names", () => {
    render(
      <PlanPanel
        {...defaultProps}
        contextSources={["cwd: C:\\Work\\demo", "projectRoot: C:\\Work\\demo", "workflow:build"]}
      />,
    );
    const footer = screen.getByText(/Looked at:/);
    expect(footer).toHaveTextContent("Working folder C:\\Work\\demo");
    expect(footer).toHaveTextContent("Workflow build");
    expect(footer).not.toHaveTextContent("projectRoot");
    expect(footer.textContent).not.toMatch(/\bcwd\b/);
  });

  it("names a different project folder", () => {
    render(
      <PlanPanel
        {...defaultProps}
        contextSources={["cwd: C:\\Work\\demo", "projectRoot: C:\\Work\\other"]}
      />,
    );
    expect(screen.getByText(/Looked at:/)).toHaveTextContent("Project folder C:\\Work\\other");
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
    expect(screen.getByText("Practice plan — Ollama is not connected. This is not a real plan.")).toBeInTheDocument();
    expect(screen.getByText("Session 1")).toBeInTheDocument();
    expect(screen.getByText("Looked at: Working folder /work, Workflow Ship")).toBeInTheDocument();
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
    expect(screen.getByText("Deletes files: cannot be undone")).toBeInTheDocument();
    expect(screen.getByText("Runs with higher permissions")).toBeInTheDocument();
    expect(screen.getByText("Uses the network")).toBeInTheDocument();
    expect(screen.getByText("This needs a careful look before it runs")).toBeInTheDocument();
    expect(screen.getByText(/which repo/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Plan" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save Workflow" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Remove hidden characters" }));
    // A destructive plan still needs the typed phrase once the hidden characters are gone.
    expect(screen.getByRole("button", { name: "Run Plan" })).toBeDisabled();
    await userEvent.type(screen.getByLabelText("Type confirm to run this"), "confirm");
    await userEvent.click(screen.getByRole("button", { name: "Run Plan" }));
    expect(onApprove).toHaveBeenCalledWith("git status");
  });
});
