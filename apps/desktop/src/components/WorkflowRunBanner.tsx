import { useWorkflowRunStore } from "@commandui/state";

export function WorkflowRunBanner() {
  const activeRun = useWorkflowRunStore((s) => s.activeRun);

  if (!activeRun) return null;

  const current = activeRun.steps[activeRun.currentStepIndex];
  const total = activeRun.steps.length;
  const stepNum = activeRun.currentStepIndex + 1;

  return (
    <div className="workflow-run-banner">
      <div className="workflow-run-header">
        <span className="workflow-run-name">{activeRun.workflowName}</span>
        <span className="workflow-run-progress">
          Step {stepNum}/{total}: {current?.command ?? "..."}
        </span>
      </div>
      <div className="workflow-run-step-dots">
        {activeRun.steps.map((step) => (
          <div
            key={step.index}
            className={`wf-dot wf-dot--${step.status}`}
            title={`Step ${step.index + 1}: ${step.command} (${step.status})`}
          />
        ))}
      </div>
    </div>
  );
}
