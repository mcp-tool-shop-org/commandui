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
  return (
    <div className="session-tabs">
      <div className="session-tab-list">
        {sessions.map((session) => {
          const exited = exitedSessionIds?.has(session.id) ?? false;
          return (
          <div
            key={session.id}
            className={`session-tab ${session.id === activeSessionId ? "active" : ""}${exited ? " exited" : ""}`}
          >
            <button
              type="button"
              className="session-tab-label"
              onClick={() => onSelect(session.id)}
            >
              {session.label}
              {exited ? " (exited)" : ""}
            </button>
            <button
              type="button"
              className="session-close"
              onClick={() => onClose(session.id)}
            >
              ×
            </button>
          </div>
          );
        })}
      </div>
      <button type="button" className="session-new" onClick={onCreate}>
        + New Session
      </button>
    </div>
  );
}
