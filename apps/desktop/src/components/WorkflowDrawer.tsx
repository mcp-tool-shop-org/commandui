import type { Workflow } from "@commandui/domain";

type Props = {
  isOpen: boolean;
  workflows: Workflow[];
  onClose: () => void;
  onRun: (workflow: Workflow) => void;
};

export function WorkflowDrawer({
  isOpen,
  workflows,
  onClose,
  onRun,
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
          workflows.map((wf) => (
            <div key={wf.id} className="history-item">
              <div className="history-main">{wf.label}</div>
              <div className="history-sub">
                <code>{wf.command}</code>
              </div>
              {wf.originalIntent && (
                <div className="history-sub muted">{wf.originalIntent}</div>
              )}
              <div className="history-sub muted">{wf.createdAt}</div>
              <button type="button" onClick={() => onRun(wf)}>
                Run
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
