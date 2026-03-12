import type { HistoryItem } from "@commandui/domain";

type Props = {
  isOpen: boolean;
  items: HistoryItem[];
  onClose: () => void;
  onRerun: (item: HistoryItem) => void;
  onReopenPlan: (item: HistoryItem) => void;
  onSaveWorkflow: (item: HistoryItem) => void;
};

const STATUS_COLORS: Record<string, string> = {
  planned: "#8dc4ff",
  success: "#8de0a8",
  failure: "#ffb4c0",
  rejected: "#ffb4c0",
};

export function HistoryDrawer({
  isOpen,
  items,
  onClose,
  onRerun,
  onReopenPlan,
  onSaveWorkflow,
}: Props) {
  if (!isOpen) return null;

  return (
    <div className="history-overlay" onClick={onClose}>
      <div className="history-drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <strong>History</strong>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        {items.length === 0 ? (
          <p className="muted">No history yet.</p>
        ) : (
          items.map((item) => (
            <div key={item.id} className="history-item">
              <div className="history-row">
                <span className="history-source">{item.source}</span>
                <span
                  className="history-status"
                  style={{ color: STATUS_COLORS[item.status] ?? "#98a2b3" }}
                >
                  {item.status}
                </span>
              </div>

              <div className="history-main">{item.userInput}</div>

              {item.generatedCommand && (
                <div className="history-sub">
                  Generated: <code>{item.generatedCommand}</code>
                </div>
              )}

              {item.executedCommand && (
                <div className="history-sub">
                  Executed: <code>{item.executedCommand}</code>
                </div>
              )}

              <div className="history-sub muted">{item.createdAt}</div>

              <div className="history-actions">
                <button
                  type="button"
                  disabled={!item.executedCommand && !item.generatedCommand}
                  onClick={() => onRerun(item)}
                >
                  Rerun
                </button>
                {item.source === "semantic" && item.generatedCommand && (
                  <button type="button" onClick={() => onReopenPlan(item)}>
                    Reopen Plan
                  </button>
                )}
                <button
                  type="button"
                  disabled={!item.executedCommand && !item.generatedCommand}
                  onClick={() => onSaveWorkflow(item)}
                >
                  Save Workflow
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
