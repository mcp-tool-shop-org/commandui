import type { MemorySuggestion } from "@commandui/domain";
import { FoldPanel } from "./FoldPanel";

type Props = {
  suggestions: MemorySuggestion[];
  onAccept: (id: string) => void;
  onDismiss: (id: string) => void;
};

const KIND_LABELS: Record<string, string> = {
  preferred_cwd: "Preferred workspace",
  recurring_command: "Frequent command",
  workflow_pattern: "Workflow pattern",
  tool_preference: "Tool preference",
  preferred_mode: "Preferred mode",
  accepted_substitution: "Command substitution",
  common_directory: "Common directory",
  preferred_package_manager: "Package manager",
  preferred_search_tool: "Search tool",
  preferred_test_command: "Test command",
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
                {KIND_LABELS[s.kind] ?? s.kind}
              </div>
              <div className="memory-label">{s.label}</div>
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
