import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Workflow, WorkflowRun } from "@commandui/domain";
import { WorkflowDrawer } from "./WorkflowDrawer";

function workflow(partial: Partial<Workflow> & Pick<Workflow, "id" | "label" | "command">): Workflow {
  return { source: "raw", createdAt: "2026-10-04T00:00:00Z", ...partial };
}

function run(partial: Partial<WorkflowRun> & Pick<WorkflowRun, "id" | "workflowId" | "status">): WorkflowRun {
  return {
    workflowName: "Ship",
    startedAt: Date.now() - 90_000,
    finishedAt: Date.now() - 30_000,
    currentStepIndex: 0,
    steps: [
      { index: 0, command: "git status", status: "success", startedAt: 1_000, finishedAt: 1_400 },
    ],
    ...partial,
  };
}

function renderDrawer(
  workflows: Workflow[],
  lastRun: Record<string, WorkflowRun> = {},
  extra: Partial<Parameters<typeof WorkflowDrawer>[0]> = {},
) {
  const props = {
    isOpen: true,
    workflows,
    lastRunByWorkflowId: lastRun,
    expandedRunWorkflowId: null as string | null,
    onClose: vi.fn(),
    onRun: vi.fn(),
    onDelete: vi.fn(),
    onExpandRun: vi.fn(),
    onRetryStep: vi.fn(),
    onCopyCommand: vi.fn(),
    onViewHistoryItem: vi.fn(),
    ...extra,
  };
  render(<WorkflowDrawer {...props} />);
  return props;
}

describe("WorkflowDrawer", () => {
  it("renders nothing while it is closed", () => {
    renderDrawer([], {}, { isOpen: false });
    expect(screen.queryByText("Workflows")).toBeNull();
  });

  it("shows skeletons while workflows are loading", () => {
    renderDrawer([], {}, { loading: true });
    expect(document.querySelectorAll(".skeleton-item")).toHaveLength(3);
  });

  it("says when nothing is saved", () => {
    renderDrawer([]);
    expect(screen.getByText(/Workflows are saved lists of commands/)).toBeInTheDocument();
  });

  it("runs and deletes a workflow, and lists its steps or its single command", async () => {
    const user = userEvent.setup();
    const props = renderDrawer([
      workflow({
        id: "w1",
        label: "Ship",
        command: "echo fallback",
        source: "promoted",
        originalIntent: "ship the branch",
        steps: [{ command: "git status" }, { command: "git diff", label: "look" }],
      }),
      workflow({ id: "w2", label: "Echo", command: "echo hi" }),
    ]);

    expect(screen.getByText("From a suggestion")).toBeInTheDocument();
    expect(screen.getByText("git status")).toBeInTheDocument();
    expect(screen.getByText("echo hi")).toBeInTheDocument();
    expect(screen.getByText("ship the branch")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Run" })[1]);
    expect(props.onRun).toHaveBeenCalledWith(expect.objectContaining({ id: "w2" }));
    await user.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    expect(props.onDelete).toHaveBeenCalledWith("w1");

    await user.click(screen.getByText("Workflows"));
    expect(props.onClose).not.toHaveBeenCalled();
    await user.click(document.querySelector(".settings-overlay")!);
    expect(props.onClose).toHaveBeenCalledOnce();
  });

  it("hides delete when the caller does not offer it", () => {
    renderDrawer([workflow({ id: "w", label: "Echo", command: "echo hi" })], {}, { onDelete: undefined });
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
  });

  it("summarises a finished run and expands it from the pointer or the keyboard", async () => {
    const user = userEvent.setup();
    const failed = run({
      id: "r",
      workflowId: "w",
      status: "failed",
      startedAt: Date.now() - 3_700_000,
      finishedAt: Date.now() - 3_600_000,
      steps: [
        { index: 0, command: "git status", status: "success", startedAt: 0, finishedAt: 500 },
        { index: 1, command: "git push", status: "failed", historyItemId: "h9", startedAt: 500, finishedAt: 2_500 },
        { index: 2, command: "echo skip", status: "skipped" },
      ],
    });
    const props = renderDrawer(
      [workflow({ id: "w", label: "Ship", command: "git push", steps: [{ command: "git status" }] })],
      { w: failed },
    );

    const summary = screen.getByRole("button", { name: /Last run:/ });
    expect(summary).toHaveAttribute("aria-expanded", "false");
    expect(summary.textContent).toMatch(/1\/3 succeeded, failed on step 2/);
    expect(summary.textContent).toMatch(/hour ago|minutes ago|just now/);

    await user.click(summary);
    expect(props.onExpandRun).toHaveBeenCalledWith("w");
    summary.focus();
    await user.keyboard(" ");
    expect(props.onExpandRun).toHaveBeenCalledWith("w");
    await user.keyboard("{Enter}");
    expect(props.onExpandRun).toHaveBeenLastCalledWith("w");
  });

  it("offers copy, retry, history, and rerun for an expanded failed run", async () => {
    const user = userEvent.setup();
    const failed = run({
      id: "r",
      workflowId: "w",
      status: "failed",
      finishedAt: Date.now() - 5_000,
      steps: [
        { index: 0, command: "git status", status: "success", startedAt: 10, finishedAt: 40 },
        { index: 1, command: "git push", status: "failed", historyItemId: "h9" },
      ],
    });
    const props = renderDrawer(
      [workflow({ id: "w", label: "Ship", command: "git push" })],
      { w: failed },
      { expandedRunWorkflowId: "w" },
    );

    expect(screen.getByText(/Started:/)).toBeInTheDocument();
    expect(screen.getByText(/Duration:/)).toBeInTheDocument();
    expect(screen.getByText("30ms")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Copy" })[1]);
    expect(props.onCopyCommand).toHaveBeenCalledWith("git push");
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(props.onRetryStep).toHaveBeenCalledWith("git push");
    await user.click(screen.getByRole("button", { name: "History" }));
    expect(props.onViewHistoryItem).toHaveBeenCalledWith("h9");
    await user.click(screen.getByRole("button", { name: "Rerun Workflow" }));
    expect(props.onRun).toHaveBeenCalledWith(expect.objectContaining({ id: "w" }));
    await user.click(screen.getByRole("button", { name: "Retry Failed Step" }));
    expect(props.onRetryStep).toHaveBeenCalledWith("git push");

    await user.click(screen.getByRole("button", { name: /Last run:/ }));
    expect(props.onExpandRun).toHaveBeenCalledWith(null);
  });

  it("describes a successful run and an interrupted one", () => {
    const success = run({
      id: "ok",
      workflowId: "a",
      status: "success",
      finishedAt: Date.now() - 10,
      steps: [{ index: 0, command: "echo hi", status: "success", startedAt: 0, finishedAt: 2_500 }],
    });
    const interrupted = run({
      id: "stop",
      workflowId: "b",
      status: "interrupted",
      finishedAt: undefined,
      steps: [
        { index: 0, command: "ssh host", status: "interrupted" },
        { index: 1, command: "echo later", status: "skipped" },
      ],
    });
    const { unmount } = render(
      <WorkflowDrawer
        isOpen
        workflows={[workflow({ id: "a", label: "Ok", command: "echo hi" })]}
        lastRunByWorkflowId={{ a: success }}
        expandedRunWorkflowId={null}
        onClose={vi.fn()}
        onRun={vi.fn()}
        onExpandRun={vi.fn()}
        onRetryStep={vi.fn()}
        onCopyCommand={vi.fn()}
        onViewHistoryItem={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: /Last run:/ }).textContent).toMatch(/1\/1 succeeded/);
    expect(screen.getByRole("button", { name: /Last run:/ }).textContent).toMatch(/2\.5s|just now/);
    unmount();

    render(
      <WorkflowDrawer
        isOpen
        workflows={[workflow({ id: "b", label: "Stop", command: "ssh host" })]}
        lastRunByWorkflowId={{ b: { ...interrupted, finishedAt: Date.now() - 86_400_000 * 2 } }}
        expandedRunWorkflowId="b"
        onClose={vi.fn()}
        onRun={vi.fn()}
        onExpandRun={vi.fn()}
        onRetryStep={vi.fn()}
        onCopyCommand={vi.fn()}
        onViewHistoryItem={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: /Last run:/ }).textContent).toMatch(/Interrupted during step 1; 1 skipped/);
    expect(screen.getByRole("button", { name: /Last run:/ }).textContent).toMatch(/days ago/);
    expect(screen.queryByRole("button", { name: "Retry Failed Step" })).toBeNull();
  });
});
