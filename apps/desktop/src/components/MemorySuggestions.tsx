import type { MemorySuggestion } from "@commandui/domain";

type Props = {
  suggestions: MemorySuggestion[];
  onAccept: (id: string) => void;
  onDismiss: (id: string) => void;
};

export function MemorySuggestions({ suggestions, onAccept, onDismiss }: Props) {
  if (suggestions.length === 0) return null;

  return (
    <div className="memory-panel">
      <span className="plan-label">Memory Suggestions</span>
      {suggestions.map((s) => (
        <div key={s.id} className="memory-item">
          <div className="memory-label">{s.label}</div>
          <div className="muted">
            {s.kind} → {s.proposedValue}
          </div>
          <div className="memory-actions">
            <button type="button" onClick={() => onAccept(s.id)}>
              Accept
            </button>
            <button type="button" onClick={() => onDismiss(s.id)}>
              Dismiss
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
