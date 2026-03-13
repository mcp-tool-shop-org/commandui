import type { Workflow, WorkflowRun } from "@commandui/domain";

type Props = {
  isOpen: boolean;
  workflows: Workflow[];
  lastRunByWorkflowId: Record<string, WorkflowRun>;
  onClose: () => void;
  onRun: (workflow: Workflow) => void;
  onDelete?: (workflowId: string) => void;
};

function formatTimeAgo(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

function formatRunSummary(run: WorkflowRun): string {
  const succeeded = run.steps.filter((s) => s.status === "success").length;
  const total = run.steps.length;
  const duration =
    run.finishedAt != null ? formatDuration(run.finishedAt - run.startedAt) : "";
  const durSuffix = duration ? ` (${duration})` : "";

  if (run.status === "success") {
    return `${succeeded}/${total} succeeded${durSuffix}`;
  }
  if (run.status === "failed") {
    const failedStep = run.steps.find((s) => s.status === "failed");
    return `${succeeded}/${total} succeeded, failed on step ${(failedStep?.index ?? 0) + 1}${durSuffix}`;
  }
  // interrupted
  const skipped = run.steps.filter((s) => s.status === "skipped").length;
  const interruptedStep = run.steps.find((s) => s.status === "interrupted");
  return `Interrupted during step ${(interruptedStep?.index ?? 0) + 1}; ${skipped} skipped${durSuffix}`;
}

export function WorkflowDrawer({
  isOpen,
  workflows,
  lastRunByWorkflowId,
  onClose,
  onRun,
  onDelete,
}: Props) {
  if (!isOpen) return null;

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <strong>Workflows</strong>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        {workflows.length === 0 ? (
          <p className="muted">No saved workflows yet.</p>
        ) : (
          workflows.map((wf) => {
            const lastRun = lastRunByWorkflowId[wf.id];
            return (
              <div key={wf.id} className="history-item">
                <div className="workflow-header">
                  <div className="history-main">{wf.label}</div>
                  {wf.source === "promoted" && (
                    <span className="workflow-badge-promoted">promoted</span>
                  )}
                </div>

                {wf.steps && wf.steps.length > 0 ? (
                  <div className="workflow-steps">
                    {wf.steps.map((step, i) => (
                      <div key={i} className="workflow-step">
                        <span className="workflow-step-num">{i + 1}</span>
                        <code>{step.command}</code>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="history-sub">
                    <code>{wf.command}</code>
                  </div>
                )}

                {lastRun && lastRun.finishedAt != null && (
                  <div className="workflow-last-run">
                    <span className={`workflow-last-run-dot workflow-last-run-dot--${lastRun.status}`} />
                    Last run: {formatRunSummary(lastRun)} — {formatTimeAgo(lastRun.finishedAt)}
                  </div>
                )}

                {wf.originalIntent && (
                  <div className="history-sub muted">{wf.originalIntent}</div>
                )}
                <div className="history-sub muted">{wf.createdAt}</div>
                <div className="workflow-actions">
                  <button type="button" onClick={() => onRun(wf)}>
                    Run
                  </button>
                  {onDelete && (
                    <button
                      type="button"
                      className="btn-danger"
                      onClick={() => onDelete(wf.id)}
                    >
                      Delete
                    </button>
                  )}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
