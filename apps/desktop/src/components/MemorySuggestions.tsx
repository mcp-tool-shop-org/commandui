import type { MemorySuggestion } from "@commandui/domain";
import { memoryKindLabel, suggestionLabel } from "../lib/memoryLabels";
import { FoldPanel } from "./FoldPanel";

type Props = {
  suggestions: MemorySuggestion[];
  onAccept: (id: string) => void;
  onDismiss: (id: string) => void;
};

export function MemorySuggestions({ suggestions, onAccept, onDismiss }: Props) {
  const pending = suggestions.filter((s) => s.status === "pending");
  if (pending.length === 0) return null;

  return (
    <FoldPanel hideLabel="Hide memory suggestions" showLabel="Show memory suggestions">
      <div className="memory-panel" role="region" aria-label="Memory suggestions" tabIndex={0}>
        <span className="plan-label">Memory suggestions</span>
        {pending.map((s) => {
          const evidenceCount = s.derivedFromHistoryIds.length;
          const confidencePct = Math.round(s.confidence * 100);

          return (
            <div key={s.id} className="memory-item">
              <div className="memory-kind-badge">
                {memoryKindLabel(s.kind)}
              </div>
              <div className="memory-label">{suggestionLabel(s.kind, s.label, s.proposedValue)}</div>
              <div className="memory-evidence">
                {evidenceCount > 0 && (
                  <span>
                    Seen in {evidenceCount} {evidenceCount === 1 ? "command" : "commands"} you ran.
                  </span>
                )}
                <span className="memory-confidence-wrap" aria-hidden="true">
                  <span
                    className={`memory-confidence-bar memory-confidence-bar--${Math.round(confidencePct / 10) * 10}`}
                  />
                </span>
                <span className="memory-confidence-pct">CommandUI is {confidencePct}% sure.</span>
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
          );
        })}
      </div>
    </FoldPanel>
  );
}
