import type { MemorySuggestion } from "@commandui/domain";

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
    <div className="memory-panel">
      <span className="plan-label">Memory Suggestions</span>
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
                <span>Based on {evidenceCount} executions</span>
              )}
              <span className="memory-confidence-wrap">
                <span
                  className="memory-confidence-bar"
                  style={{ width: `${confidencePct}%` }}
                />
              </span>
              <span className="memory-confidence-pct">{confidencePct}%</span>
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
  );
}
