import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useFocusStore } from "@commandui/state";

export type PlanRisk = "low" | "medium" | "high";

/** Latest edited command and checkbox, reported so shortcuts share canRun. */
export type PlanRunGate = {
  command: string;
  confirmed: boolean;
};

export function planNeedsConfirmation(
  risk: PlanRisk,
  requireMediumRiskConfirmation: boolean,
): boolean {
  return risk === "high" || (risk === "medium" && requireMediumRiskConfirmation);
}

/** Same rule as the Run Plan button: non-empty command, and the checkbox when required. */
export function planCanRun(input: {
  command: string;
  risk: PlanRisk;
  requireMediumRiskConfirmation: boolean;
  confirmed: boolean;
}): boolean {
  const needsConfirmation = planNeedsConfirmation(
    input.risk,
    input.requireMediumRiskConfirmation,
  );
  return input.command.trim().length > 0 && (!needsConfirmation || input.confirmed);
}

type Props = {
  sessionId: string;
  intent: string;
  command: string;
  risk: PlanRisk;
  explanation: string;
  contextSources?: string[];
  plannerSource?: string;
  requireMediumRiskConfirmation?: boolean;
  onRunGate?: (gate: PlanRunGate) => void;
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
  contextSources,
  plannerSource,
  requireMediumRiskConfirmation = true,
  onRunGate,
  onApprove,
  onReject,
  onSaveWorkflow,
}: Props) {
  const [editedCommand, setEditedCommand] = useState(command);
  const [confirmRisk, setConfirmRisk] = useState(false);
  const [commandSync, setCommandSync] = useState(command);
  const panelRef = useRef<HTMLDivElement>(null);
  const commandTextareaRef = useRef<HTMLTextAreaElement>(null);
  const setFocusZone = useFocusStore((s) => s.setFocusZone);

  // Reset the draft and the checkbox when a different command is shown,
  // before paint, so a previous confirmation cannot approve the new plan.
  if (command !== commandSync) {
    setCommandSync(command);
    setEditedCommand(command);
    setConfirmRisk(false);
  }

  /** Focus the command edit textarea (for keyboard shortcut "E") */
  function focusCommandEdit() {
    commandTextareaRef.current?.focus();
  }

  // Expose focusCommandEdit on the panel ref for AppShell shortcut wiring
  useEffect(() => {
    const el = panelRef.current;
    if (el) (el as unknown as Record<string, unknown>).__focusEdit = focusCommandEdit;
  });

  useLayoutEffect(() => {
    onRunGate?.({
      command: command ? editedCommand.trim() : "",
      confirmed: command ? confirmRisk : false,
    });
  }, [command, editedCommand, confirmRisk, onRunGate]);

  if (!command) {
    return (
      <div className="plan-panel">
        <p className="muted">No semantic plan yet.</p>
      </div>
    );
  }

  const needsConfirmation = planNeedsConfirmation(risk, requireMediumRiskConfirmation);
  const canRun = planCanRun({
    command: editedCommand,
    risk,
    requireMediumRiskConfirmation,
    confirmed: confirmRisk,
  });

  return (
    <div
      ref={panelRef}
      className="plan-panel"
      tabIndex={0}
      onFocus={() => setFocusZone("plan")}
    >
      {plannerSource === "mock" && (
        <div className="plan-mock-notice muted">
          Mock planner — Ollama not connected
        </div>
      )}

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

      {contextSources && contextSources.length > 0 && (
        <div className="plan-context-sources">
          Context: {contextSources.join(", ")}
        </div>
      )}
    </div>
  );
}
