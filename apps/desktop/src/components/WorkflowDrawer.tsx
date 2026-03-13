import type { Workflow, WorkflowRun, WorkflowStepRun } from "@commandui/domain";

type Props = {
  isOpen: boolean;
  workflows: Workflow[];
  lastRunByWorkflowId: Record<string, WorkflowRun>;
  expandedRunWorkflowId: string | null;
  onClose: () => void;
  onRun: (workflow: Workflow) => void;
  onDelete?: (workflowId: string) => void;
  onExpandRun: (workflowId: string | null) => void;
  onRetryStep: (command: string) => void;
  onCopyCommand: (command: string) => void;
  onViewHistoryItem: (historyItemId: string) => void;
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

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleTimeString();
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
  const skipped = run.steps.filter((s) => s.status === "skipped").length;
  const interruptedStep = run.steps.find((s) => s.status === "interrupted");
  return `Interrupted during step ${(interruptedStep?.index ?? 0) + 1}; ${skipped} skipped${durSuffix}`;
}

function stepStatusClass(step: WorkflowStepRun): string {
  if (step.status === "failed") return " workflow-step-row--failed";
  if (step.status === "interrupted") return " workflow-step-row--interrupted";
  return "";
}

function stepDuration(step: WorkflowStepRun): string {
  if (step.startedAt && step.finishedAt) {
    return formatDuration(step.finishedAt - step.startedAt);
  }
  return "";
}

export function WorkflowDrawer({
  isOpen,
  workflows,
  lastRunByWorkflowId,
  expandedRunWorkflowId,
  onClose,
  onRun,
  onDelete,
  onExpandRun,
  onRetryStep,
  onCopyCommand,
  onViewHistoryItem,
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
            const isExpanded = expandedRunWorkflowId === wf.id;
            const failedStep = lastRun?.steps.find((s) => s.status === "failed");

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
                  <div
                    className="workflow-last-run"
                    data-expandable=""
                    onClick={() => onExpandRun(isExpanded ? null : wf.id)}
                  >
                    <span className={`workflow-last-run-dot workflow-last-run-dot--${lastRun.status}`} />
                    {isExpanded ? "▼" : "▶"} Last run: {formatRunSummary(lastRun)} — {formatTimeAgo(lastRun.finishedAt)}
                  </div>
                )}

                {isExpanded && lastRun && (
                  <div className="workflow-run-detail">
                    <div className="workflow-run-detail-header">
                      <span className={`wf-dot wf-dot--${lastRun.status}`} />
                      <span>Started: {formatTimestamp(lastRun.startedAt)}</span>
                      {lastRun.finishedAt != null && (
                        <span>Duration: {formatDuration(lastRun.finishedAt - lastRun.startedAt)}</span>
                      )}
                    </div>

                    {lastRun.steps.map((step) => (
                      <div
                        key={step.index}
                        className={`workflow-step-row${stepStatusClass(step)}`}
                      >
                        <span className="workflow-step-num">{step.index + 1}</span>
                        <span className={`wf-dot wf-dot--${step.status}`} />
                        <code title={step.command}>{step.command}</code>
                        <span className="muted">{stepDuration(step)}</span>
                        <div className="step-actions">
                          <button
                            type="button"
                            onClick={() => onCopyCommand(step.command)}
                            title="Copy command"
                          >
                            Copy
                          </button>
                          {step.status === "failed" && (
                            <button
                              type="button"
                              onClick={() => onRetryStep(step.command)}
                              title="Load into composer"
                            >
                              Retry
                            </button>
                          )}
                          {step.historyItemId && (
                            <button
                              type="button"
                              className="link-btn"
                              onClick={() => onViewHistoryItem(step.historyItemId!)}
                              title="View in history"
                            >
                              History
                            </button>
                          )}
                        </div>
                      </div>
                    ))}

                    <div className="workflow-run-detail-actions">
                      <button type="button" onClick={() => onRun(wf)}>
                        Rerun Workflow
                      </button>
                      {failedStep && (
                        <button
                          type="button"
                          onClick={() => onRetryStep(failedStep.command)}
                        >
                          Retry Failed Step
                        </button>
                      )}
                    </div>
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
