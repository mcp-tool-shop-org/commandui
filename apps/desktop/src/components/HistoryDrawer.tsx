import { useEffect, useState } from "react";
import type { HistoryItem } from "@commandui/domain";
import type { SessionSummary } from "@commandui/domain";
import { historyStatusLabel } from "../lib/commandResult";
import { useModalDialog } from "../lib/useModalDialog";
import { RelativeTime } from "./RelativeTime";

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
  /** Selected rows, oldest first, so the saved list runs in the order they happened. */
  onSaveSelected?: (items: HistoryItem[]) => void;
  onCopyCommand: (command: string) => void;
  onViewWorkflowRun?: (workflowRunId: string) => void;
  initialExpandedId?: string | null;
  loading?: boolean;
};

function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms < 0) return "";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60_000);
  const secs = Math.round((ms % 60_000) / 1000);
  return `${mins}m ${secs}s`;
}

function commandOf(item: HistoryItem): string | undefined {
  return item.executedCommand ?? item.generatedCommand;
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
  onSaveSelected,
  onCopyCommand,
  onViewWorkflowRun,
  initialExpandedId,
  loading = false,
}: Props) {
  const [search, setSearch] = useState("");
  const [sessionFilter, setSessionFilter] = useState<string>("current");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [lastCount, setLastCount] = useState("3");

  // Allow AppShell to programmatically expand a specific history item
  useEffect(() => {
    if (initialExpandedId) {
      setExpandedId(initialExpandedId);
    }
  }, [initialExpandedId]);

  const dialogRef = useModalDialog(isOpen, onClose);

  if (!isOpen) return null;

  // Determine which items to show based on session filter
  const baseItems =
    sessionFilter === "all"
      ? allItems
      : sessionFilter === "current"
        ? items
        : allItems.filter((item) => item.sessionId === sessionFilter);

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

  // The list is newest first. The first N are the most recent commands.
  function selectLast() {
    const count = Math.max(1, Math.floor(Number(lastCount)) || 1);
    const ids = filtered
      .filter((item) => Boolean(commandOf(item)))
      .slice(0, count)
      .map((item) => item.id);
    setSelected(new Set(ids));
  }

  function saveSelected() {
    if (!onSaveSelected) return;
    const chosen = filtered
      .filter((item) => selected.has(item.id) && Boolean(commandOf(item)))
      .slice()
      .reverse();
    if (chosen.length === 0) return;
    onSaveSelected(chosen);
  }

  function toggleSelected(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return (
    <div className="history-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className="history-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="history-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="drawer-header">
          <strong id="history-title">History</strong>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>

        <div className="history-controls">
          <input
            className="history-search"
            type="text"
            aria-label="Search history"
            data-autofocus
            placeholder="Search history…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <select
            className="history-filter"
            aria-label="Which session"
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

        {loading ? (
          <>
            {[1, 2, 3].map((i) => (
              <div key={i} className="skeleton-item">
                <div className="skeleton skeleton-text short" />
                <div className="skeleton skeleton-text" style={{ width: "40%" }} />
              </div>
            ))}
          </>
        ) : filtered.length === 0 ? (
          <p className="muted">No history yet.</p>
        ) : (
          <>
            <div className="history-save-bar">
              <label htmlFor="history-last-n">Last commands</label>
              <input
                id="history-last-n"
                className="history-last-n"
                type="number"
                min={1}
                value={lastCount}
                onChange={(e) => setLastCount(e.target.value)}
              />
              <button type="button" onClick={selectLast}>
                Select last
              </button>
              <button type="button" onClick={saveSelected} disabled={selected.size === 0 || !onSaveSelected}>
                Save as workflow
              </button>
            </div>
            {filtered.map((item) => {
            const isExpanded = expandedId === item.id;
            const command = item.executedCommand ?? item.generatedCommand;
            const duration = formatDuration(item.durationMs);
            const sourceLabel =
              item.source === "semantic"
                ? item.plannerSource === "mock"
                  ? "From Ask, practice plan"
                  : "From Ask"
                : "Typed command";

            return (
              <div
                key={item.id}
                className={`history-item${isExpanded ? " history-item-expanded" : ""}`}
              >
                <div className="history-row">
                  <input
                    className="history-select"
                    type="checkbox"
                    aria-label={`Select ${item.userInput}`}
                    checked={selected.has(item.id)}
                    disabled={!command}
                    onChange={() => toggleSelected(item.id)}
                  />
                  <button
                    type="button"
                    className="history-expand"
                    aria-expanded={isExpanded}
                    onClick={() => setExpandedId(isExpanded ? null : item.id)}
                  >
                    <span aria-hidden="true">{isExpanded ? "▼" : "▶"}</span>{" "}
                    <span className="history-main">{item.userInput}</span>
                  </button>
                  <span className={`history-status history-status--${item.status}`}>
                    {historyStatusLabel(item.status)}
                  </span>
                </div>

                <div className="history-meta">
                  <span className="history-source">{sourceLabel}</span>
                  {item.workflowRunId && onViewWorkflowRun && (
                    <span
                      className="history-wf-badge"
                      onClick={(e) => {
                        e.stopPropagation();
                        onViewWorkflowRun(item.workflowRunId!);
                      }}
                    >
                      WF
                    </span>
                  )}
                  {duration && (
                    <span className="history-duration">{duration}</span>
                  )}
                  {item.cwd && (
                    <span className="history-cwd">{item.cwd}</span>
                  )}
                  <span className="history-time">
                    <RelativeTime value={item.createdAt} />
                  </span>
                </div>

                {item.generatedCommand && !isExpanded && (
                  <div className="history-sub">
                    <span aria-hidden="true">→ </span>
                    {item.generatedCommand}
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
                    {item.workflowRunId && onViewWorkflowRun && (
                      <div className="history-detail-row">
                        <span className="detail-label">Workflow</span>
                        <button
                          type="button"
                          className="link-btn"
                          onClick={() => onViewWorkflowRun(item.workflowRunId!)}
                        >
                          View workflow run
                        </button>
                      </div>
                    )}

                    <div className="history-actions">
                      <button
                        type="button"
                        disabled={!command || item.status === "rejected"}
                        title={
                          item.status === "rejected"
                            ? "This plan was rejected. Use View Plan to run it through the risk check."
                            : undefined
                        }
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
          })}
          </>
        )}
      </div>
    </div>
  );
}
