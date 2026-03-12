import type { SessionSummary } from "@commandui/domain";

type Props = {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onClose: (id: string) => void;
};

export function SessionTabs({
  sessions,
  activeSessionId,
  onSelect,
  onCreate,
  onClose,
}: Props) {
  return (
    <div className="session-tabs">
      <div className="session-tab-list">
        {sessions.map((session) => (
          <div
            key={session.id}
            className={`session-tab ${session.id === activeSessionId ? "active" : ""}`}
          >
            <button
              type="button"
              className="session-tab-label"
              onClick={() => onSelect(session.id)}
            >
              {session.label}
            </button>
            <button
              type="button"
              className="session-close"
              onClick={() => onClose(session.id)}
            >
              ×
            </button>
          </div>
        ))}
      </div>
      <button type="button" className="session-new" onClick={onCreate}>
        + New Session
      </button>
    </div>
  );
}
