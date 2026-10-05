import type { SessionSummary } from "@commandui/domain";

type Props = {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onClose: (id: string) => void;
  /** Sessions whose shell has exited; their tab is marked. */
  exitedSessionIds?: ReadonlySet<string>;
};

export function SessionTabs({
  sessions,
  activeSessionId,
  onSelect,
  onCreate,
  onClose,
  exitedSessionIds,
}: Props) {
  const tabIds = sessions.map((session) => `session-tab-${session.id}`).join(" ");
  return (
    <div className="session-tabs">
      <div className="session-tab-list">
        <div role="tablist" aria-label="Sessions" aria-owns={tabIds || undefined} className="visually-hidden" />
        {sessions.map((session) => {
          const exited = exitedSessionIds?.has(session.id) ?? false;
          const selected = session.id === activeSessionId;
          const name = session.label || "Session";
          return (
          <div
            key={session.id}
            className={`session-tab ${selected ? "active" : ""}${exited ? " exited" : ""}`}
          >
            <button
              type="button"
              id={`session-tab-${session.id}`}
              role="tab"
              className="session-tab-label"
              aria-selected={selected}
              onClick={() => onSelect(session.id)}
            >
              {session.label}
              {exited ? " (exited)" : ""}
            </button>
            <button
              type="button"
              className="session-close"
              onClick={() => onClose(session.id)}
              aria-label={`Close ${name}`}
            >
              <span aria-hidden="true">×</span>
            </button>
          </div>
          );
        })}
      </div>
      <button type="button" className="session-new" onClick={onCreate} aria-label="New session">
        <span aria-hidden="true">+</span> New Session
      </button>
    </div>
  );
}
