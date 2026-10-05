import type { MemoryItem } from "@commandui/domain";
import { memoryKindLabel, memoryScopeLabel, memoryValueLabel } from "../lib/memoryLabels";
import { useModalDialog } from "../lib/useModalDialog";

type Props = {
  isOpen: boolean;
  items: MemoryItem[];
  onClose: () => void;
  onDelete: (memoryId: string) => void;
  loading?: boolean;
};

export function MemoryDrawer({ isOpen, items, onClose, onDelete, loading = false }: Props) {
  const dialogRef = useModalDialog(isOpen, onClose);
  if (!isOpen) return null;

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className="settings-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="memory-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="drawer-header">
          <strong id="memory-title">Memory</strong>
          <button type="button" data-autofocus onClick={onClose}>
            Close
          </button>
        </div>

        {loading ? (
          <>
            {[1, 2, 3].map((i) => (
              <div key={i} className="skeleton-item">
                <div className="skeleton skeleton-text short" />
                <div className="skeleton skeleton-text" style={{ width: "40%" }} />
              </div>
            ))}
          </>
        ) : items.length === 0 ? (
          <p className="muted">No saved memory yet.</p>
        ) : (
          items.map((item) => (
            <div key={item.id} className="memory-item">
              <div className="history-row">
                <span className="history-source">{memoryKindLabel(item.kind)}</span>
                <span className="muted">{memoryScopeLabel(item.scope, item.projectRoot)}</span>
              </div>
              <div className="history-main" id={`memory-item-${item.id}`}>
                {item.key} <span aria-hidden="true">→</span> {memoryValueLabel(item.kind, item.value)}
              </div>
              <button
                type="button"
                aria-describedby={`memory-item-${item.id}`}
                onClick={() => onDelete(item.id)}
              >
                Delete
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
