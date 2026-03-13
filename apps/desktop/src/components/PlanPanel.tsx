import { useEffect, useRef, useState } from "react";
import { useFocusStore } from "@commandui/state";

type Props = {
  sessionId: string;
  intent: string;
  command: string;
  risk: "low" | "medium" | "high";
  explanation: string;
  requireMediumRiskConfirmation?: boolean;
  onApprove: (command: string) => void;
  onReject: () => void;
  onSaveWorkflow: (command: string) => void;
};

export function PlanPanel({
  sessionId,
  intent,
  command,
  risk,
  explanation,
  requireMediumRiskConfirmation = true,
  onApprove,
  onReject,
  onSaveWorkflow,
}: Props) {
  const [editedCommand, setEditedCommand] = useState(command);
  const [confirmRisk, setConfirmRisk] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const commandTextareaRef = useRef<HTMLTextAreaElement>(null);
  const setFocusZone = useFocusStore((s) => s.setFocusZone);

  useEffect(() => {
    setEditedCommand(command);
    setConfirmRisk(false);
  }, [command]);

  /** Focus the command edit textarea (for keyboard shortcut "E") */
  function focusCommandEdit() {
    commandTextareaRef.current?.focus();
  }

  // Expose focusCommandEdit on the panel ref for AppShell shortcut wiring
  useEffect(() => {
    const el = panelRef.current;
    if (el) (el as unknown as Record<string, unknown>).__focusEdit = focusCommandEdit;
  });

  if (!command) {
    return (
      <div className="plan-panel">
        <p className="muted">No semantic plan yet.</p>
      </div>
    );
  }

  const needsConfirmation =
    risk === "high" || (risk === "medium" && requireMediumRiskConfirmation);
  const canRun =
    editedCommand.trim().length > 0 && (!needsConfirmation || confirmRisk);

  return (
    <div
      ref={panelRef}
      className="plan-panel"
      tabIndex={0}
      onFocus={() => setFocusZone("plan")}
    >
      <div className="plan-section">
        <span className="plan-label">Intent</span>
        <p>{intent}</p>
      </div>

      <div className="plan-edit-block">
        <span className="plan-label">Command</span>
        <textarea
          ref={commandTextareaRef}
          className="plan-command-input"
          value={editedCommand}
          onChange={(e) => setEditedCommand(e.target.value)}
          rows={3}
        />
      </div>

      <div className="plan-section">
        <span className="plan-label">Risk</span>
        <span className={`risk-badge risk-${risk}`}>{risk}</span>
      </div>

      <div className="plan-section">
        <span className="plan-label">Explanation</span>
        <p className="muted">{explanation}</p>
      </div>

      {needsConfirmation && (
        <label className="confirm-row">
          <input
            type="checkbox"
            checked={confirmRisk}
            onChange={(e) => setConfirmRisk(e.target.checked)}
          />
          I understand the risks of this {risk}-risk command
        </label>
      )}

      <div className="plan-actions">
        <button
          type="button"
          disabled={!canRun}
          onClick={() => onApprove(editedCommand.trim())}
        >
          Run Plan
        </button>
        <button type="button" onClick={onReject}>
          Reject
        </button>
        <button
          type="button"
          onClick={() => onSaveWorkflow(editedCommand.trim())}
        >
          Save Workflow
        </button>
      </div>
    </div>
  );
}
