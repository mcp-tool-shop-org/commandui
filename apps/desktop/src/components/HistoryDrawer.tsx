import { useState } from "react";
import type { HistoryItem } from "@commandui/domain";
import type { SessionSummary } from "@commandui/domain";

type Props = {
  isOpen: boolean;
  items: HistoryItem[];
  allItems: HistoryItem[];
  sessions: SessionSummary[];
  activeSessionId: string | null;
  onClose: () => void;
  onRerun: (item: HistoryItem) => void;
  onReopenPlan: (item: HistoryItem) => void;
  onSaveWorkflow: (item: HistoryItem) => void;
  onCopyCommand: (command: string) => void;
};

const STATUS_COLORS: Record<string, string> = {
  planned: "#8dc4ff",
  success: "#8de0a8",
  failure: "#ffb4c0",
  rejected: "#ffb4c0",
  interrupted: "#ffd48d",
  unknown: "#98a2b3",
};

function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms < 0) return "";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60_000);
  const secs = Math.round((ms % 60_000) / 1000);
  return `${mins}m ${secs}s`;
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
}

export function HistoryDrawer({
  isOpen,
  items,
  allItems,
  sessions,
  activeSessionId,
  onClose,
  onRerun,
  onReopenPlan,
  onSaveWorkflow,
  onCopyCommand,
}: Props) {
  const [search, setSearch] = useState("");
  const [sessionFilter, setSessionFilter] = useState<string>("current");
  const [expandedId, setExpandedId] = useState<string | null>(null);

  if (!isOpen) return null;

  // Determine which items to show based on session filter
  const baseItems = sessionFilter === "all" ? allItems : items;

  // Apply text search
  const filtered = search
    ? baseItems.filter((item) => {
        const q = search.toLowerCase();
        return (
          item.userInput.toLowerCase().includes(q) ||
          (item.executedCommand?.toLowerCase().includes(q) ?? false) ||
          (item.generatedCommand?.toLowerCase().includes(q) ?? false)
        );
      })
    : baseItems;

  return (
    <div className="history-overlay" onClick={onClose}>
      <div className="history-drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <strong>History</strong>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        <div className="history-controls">
          <input
            className="history-search"
            type="text"
            placeholder="Search history…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <select
            className="history-filter"
            value={sessionFilter}
            onChange={(e) => setSessionFilter(e.target.value)}
          >
            <option value="current">Current Session</option>
            <option value="all">All Sessions</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
        </div>

        {filtered.length === 0 ? (
          <p className="muted">No history yet.</p>
        ) : (
          filtered.map((item) => {
            const isExpanded = expandedId === item.id;
            const command = item.executedCommand ?? item.generatedCommand;
            const duration = formatDuration(item.durationMs);
            const sourceLabel =
              item.source === "semantic"
                ? `semantic${item.plannerSource ? `/${item.plannerSource}` : ""}`
                : "raw";

            return (
              <div
                key={item.id}
                className={`history-item${isExpanded ? " history-item-expanded" : ""}`}
                onClick={() => setExpandedId(isExpanded ? null : item.id)}
              >
                <div className="history-row">
                  <span className="history-main">
                    {isExpanded ? "▼" : "▶"} {item.userInput}
                  </span>
                  <span
                    className="history-status"
                    style={{ color: STATUS_COLORS[item.status] ?? "#98a2b3" }}
                  >
                    {item.status}
                  </span>
                </div>

                <div className="history-meta">
                  <span className="history-source">{sourceLabel}</span>
                  {duration && (
                    <span className="history-duration">{duration}</span>
                  )}
                  {item.cwd && (
                    <span className="history-cwd">{item.cwd}</span>
                  )}
                  <span
                    className="history-time"
                    title={item.createdAt}
                  >
                    {relativeTime(item.createdAt)}
                  </span>
                </div>

                {item.generatedCommand && !isExpanded && (
                  <div className="history-sub">
                    → {item.generatedCommand}
                  </div>
                )}

                {isExpanded && (
                  <div
                    className="history-detail"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <div className="history-detail-row">
                      <span className="detail-label">Intent</span>
                      <span>{item.userInput}</span>
                    </div>
                    {command && (
                      <div className="history-detail-row">
                        <span className="detail-label">Command</span>
                        <code>{command}</code>
                      </div>
                    )}
                    <div className="history-detail-row">
                      <span className="detail-label">Source</span>
                      <span>{sourceLabel}</span>
                    </div>
                    {item.cwd && (
                      <div className="history-detail-row">
                        <span className="detail-label">CWD</span>
                        <span>{item.cwd}</span>
                      </div>
                    )}
                    {duration && (
                      <div className="history-detail-row">
                        <span className="detail-label">Duration</span>
                        <span>{duration}</span>
                      </div>
                    )}
                    {item.exitCode !== undefined && (
                      <div className="history-detail-row">
                        <span className="detail-label">Exit code</span>
                        <span>{item.exitCode}</span>
                      </div>
                    )}
                    <div className="history-detail-row">
                      <span className="detail-label">Time</span>
                      <span>{new Date(item.createdAt).toLocaleString()}</span>
                    </div>

                    <div className="history-actions">
                      <button
                        type="button"
                        disabled={!command}
                        onClick={() => onRerun(item)}
                      >
                        Rerun
                      </button>
                      {command && (
                        <button
                          type="button"
                          onClick={() => onCopyCommand(command)}
                        >
                          Copy
                        </button>
                      )}
                      {item.source === "semantic" && item.generatedCommand && (
                        <button
                          type="button"
                          onClick={() => onReopenPlan(item)}
                        >
                          View Plan
                        </button>
                      )}
                      <button
                        type="button"
                        disabled={!command}
                        onClick={() => onSaveWorkflow(item)}
                      >
                        Save Workflow
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
