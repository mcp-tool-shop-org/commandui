import type { MemoryItem } from "@commandui/domain";

type Props = {
  isOpen: boolean;
  items: MemoryItem[];
  onClose: () => void;
  onDelete: (memoryId: string) => void;
};

export function MemoryDrawer({ isOpen, items, onClose, onDelete }: Props) {
  if (!isOpen) return null;

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <strong>Memory</strong>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        {items.length === 0 ? (
          <p className="muted">No saved memory yet.</p>
        ) : (
          items.map((item) => (
            <div key={item.id} className="memory-item">
              <div className="history-row">
                <span className="history-source">{item.kind}</span>
                <span className="muted">{item.scope}</span>
              </div>
              <div className="history-main">
                {item.key} → {item.value}
              </div>
              {item.projectRoot && (
                <div className="history-sub muted">{item.projectRoot}</div>
              )}
              <button type="button" onClick={() => onDelete(item.id)}>
                Delete
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
