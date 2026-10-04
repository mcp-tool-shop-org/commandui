import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { useWorkflowRunStore } from "@commandui/state";
import type { WorkflowRun } from "@commandui/domain";
import { WorkflowRunBanner } from "./WorkflowRunBanner";

describe("WorkflowRunBanner", () => {
  it("renders nothing when no workflow is running", () => {
    useWorkflowRunStore.setState({ activeRun: null, lastRunByWorkflowId: {} });
    const { container } = render(<WorkflowRunBanner />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the current step and a dot for every step", () => {
    const run: WorkflowRun = {
      id: "r",
      workflowId: "w",
      workflowName: "Ship",
      startedAt: 1,
      status: "running",
      currentStepIndex: 1,
      steps: [
        { index: 0, command: "git status", status: "success" },
        { index: 1, command: "git diff", status: "running" },
      ],
    };
    useWorkflowRunStore.setState({ activeRun: run, lastRunByWorkflowId: {} });
    render(<WorkflowRunBanner />);
    expect(screen.getByText("Ship")).toBeInTheDocument();
    expect(screen.getByText("Step 2/2: git diff")).toBeInTheDocument();
    expect(document.querySelectorAll(".wf-dot")).toHaveLength(2);
  });

  it("shows a placeholder when the current step is missing", () => {
    const run: WorkflowRun = {
      id: "r",
      workflowId: "w",
      workflowName: "Ship",
      startedAt: 1,
      status: "running",
      currentStepIndex: 3,
      steps: [{ index: 0, command: "echo hi", status: "pending" }],
    };
    useWorkflowRunStore.setState({ activeRun: run, lastRunByWorkflowId: {} });
    render(<WorkflowRunBanner />);
    expect(screen.getByText("Step 4/1: ...")).toBeInTheDocument();
  });
});
