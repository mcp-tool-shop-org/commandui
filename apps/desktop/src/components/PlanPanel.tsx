import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { displayPath } from "../lib/displayPath";
import { useFocusStore } from "@commandui/state";
import {
  describeHiddenChars,
  hasHiddenChars,
  markHiddenChars,
  stripHiddenChars,
} from "../lib/displaySafe";

export type PlanRisk = "low" | "medium" | "high";

/** Latest edited command and checkbox, reported so shortcuts share canRun. */
export type PlanRunGate = {
  command: string;
  confirmed: boolean;
};

/** Flags the planner reported next to `risk`; any one of them gates the run like high risk. */
export type PlanSafetyFlags = {
  requiresConfirmation?: boolean;
  destructive?: boolean;
  escalatesPrivileges?: boolean;
};

export function planNeedsConfirmation(
  risk: PlanRisk,
  requireMediumRiskConfirmation: boolean,
  flags?: PlanSafetyFlags,
): boolean {
  return (
    risk === "high" ||
    (risk === "medium" && requireMediumRiskConfirmation) ||
    flags?.requiresConfirmation === true ||
    flags?.destructive === true ||
    flags?.escalatesPrivileges === true
  );
}

/**
 * Same rule as the Run Plan button: non-empty command with no hidden or
 * direction-changing characters, and the checkbox when required.
 */
export function planCanRun(input: {
  command: string;
  risk: PlanRisk;
  requireMediumRiskConfirmation: boolean;
  confirmed: boolean;
  flags?: PlanSafetyFlags;
}): boolean {
  const needsConfirmation = planNeedsConfirmation(
    input.risk,
    input.requireMediumRiskConfirmation,
    input.flags,
  );
  return (
    input.command.trim().length > 0 &&
    !hasHiddenChars(input.command) &&
    (!needsConfirmation || input.confirmed)
  );
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
  /** Planner flags beyond `risk`; shown as badges and used to gate the run. */
  flags?: PlanSafetyFlags & { touchesFiles?: boolean; touchesNetwork?: boolean };
  safetyFlags?: string[];
  ambiguityFlags?: string[];
  /** The session this plan will run in. */
  target?: { label: string; cwd?: string };
  /** When set, Run is disabled and this says why (wrong tab, closed or exited session). */
  blockedReason?: string;
  /** A one-off note, e.g. that a reopened plan was moved to the open session. */
  notice?: string;
  onGoToTarget?: () => void;
  onRetarget?: () => void;
  retargetLabel?: string;
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
  flags,
  safetyFlags,
  ambiguityFlags,
  target,
  blockedReason,
  notice,
  onGoToTarget,
  onRetarget,
  retargetLabel,
  onRunGate,
  onApprove,
  onReject,
  onSaveWorkflow,
}: Props) {
  const [editedCommand, setEditedCommand] = useState(command);
  const [confirmRisk, setConfirmRisk] = useState(false);
  const [commandSync, setCommandSync] = useState(command);
  const [targetSync, setTargetSync] = useState(sessionId);
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
  // A plan moved to another session (different cwd) needs a fresh confirmation too.
  if (sessionId !== targetSync) {
    setTargetSync(sessionId);
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
      // A command with hidden characters reports as empty so no shortcut can run it.
      command: command && !hasHiddenChars(editedCommand) ? editedCommand.trim() : "",
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

  const needsConfirmation = planNeedsConfirmation(risk, requireMediumRiskConfirmation, flags);
  const hiddenWarning = describeHiddenChars(editedCommand);
  const canRun =
    !blockedReason &&
    planCanRun({
      command: editedCommand,
      risk,
      requireMediumRiskConfirmation,
      confirmed: confirmRisk,
      flags,
    });
  const flagBadges: string[] = [];
  if (flags?.destructive) flagBadges.push("destructive");
  if (flags?.escalatesPrivileges) flagBadges.push("escalates privileges");
  if (flags?.touchesFiles) flagBadges.push("touches files");
  if (flags?.touchesNetwork) flagBadges.push("uses the network");

  return (
    <div
      ref={panelRef}
      className="plan-panel"
      tabIndex={0}
      aria-label="Command plan"
      onFocus={() => setFocusZone("plan")}
    >
      {plannerSource === "mock" && (
        <div className="plan-mock-notice muted">
          Mock planner — Ollama not connected
        </div>
      )}

      {target && (
        <div className="plan-section plan-target">
          <span className="plan-label">Runs in</span>
          <p>
            <strong>{target.label}</strong>
            {target.cwd ? (
              <span className="muted" title={target.cwd}> — {displayPath(target.cwd)}</span>
            ) : null}
          </p>
        </div>
      )}

      {notice && <div className="plan-notice">{notice}</div>}

      {blockedReason && (
        <div className="plan-blocked" role="alert">
          <span>{blockedReason}</span>
          {onGoToTarget && (
            <button type="button" onClick={onGoToTarget}>
              Go to that session
            </button>
          )}
          {onRetarget && (
            <button type="button" onClick={onRetarget}>
              {retargetLabel ?? "Run in the current session instead"}
            </button>
          )}
        </div>
      )}

      <div className="plan-section">
        <span className="plan-label">Intent</span>
        <p>{intent}</p>
      </div>

      <div className="plan-edit-block">
        <label className="plan-label" htmlFor="plan-command">
          Command
        </label>
        <textarea
          id="plan-command"
          ref={commandTextareaRef}
          className="plan-command-input"
          value={editedCommand}
          onChange={(e) => setEditedCommand(e.target.value)}
          rows={3}
        />
        {hiddenWarning && (
          <div className="plan-hidden-warning" role="alert">
            <strong>Hidden characters in this command.</strong> The text above may not be
            what the shell receives: {hiddenWarning}. Run is blocked until they are removed.
            <pre className="plan-hidden-preview">{markHiddenChars(editedCommand)}</pre>
            <button
              type="button"
              onClick={() => setEditedCommand(stripHiddenChars(editedCommand))}
            >
              Remove hidden characters
            </button>
          </div>
        )}
      </div>

      <div className="plan-section">
        <span className="plan-label">Risk</span>
        <span className={`risk-badge risk-${risk}`}>{risk}</span>
        {flagBadges.map((b) => (
          <span key={b} className="risk-badge risk-flag">
            {b}
          </span>
        ))}
      </div>

      {((safetyFlags && safetyFlags.length > 0) ||
        (ambiguityFlags && ambiguityFlags.length > 0)) && (
        <div className="plan-section plan-flags">
          {safetyFlags && safetyFlags.length > 0 && (
            <p>
              <span className="plan-label">Safety</span> {safetyFlags.join("; ")}
            </p>
          )}
          {ambiguityFlags && ambiguityFlags.length > 0 && (
            <p>
              <span className="plan-label">Unclear</span> {ambiguityFlags.join("; ")}
            </p>
          )}
        </div>
      )}

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
          {risk === "high" || risk === "medium"
            ? `I understand the risks of this ${risk}-risk command`
            : "I understand the planner flagged this command (see above) and want to run it"}
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
          disabled={hasHiddenChars(editedCommand)}
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
