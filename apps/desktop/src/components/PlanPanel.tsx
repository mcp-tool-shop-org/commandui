import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { displayPath } from "../lib/displayPath";
import {
  confirmationMatches,
  confirmationPhrase,
  explainCommand,
  flagInWords,
  riskInWords,
} from "../lib/planLanguage";
import { useFocusStore } from "@commandui/state";
import {
  describeHiddenChars,
  hasHiddenChars,
  markHiddenChars,
  stripHiddenChars,
} from "../lib/displaySafe";

export type PlanRisk = "low" | "medium" | "high";

/** Latest edited command and whether the high-risk phrase was typed. */
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

/** Friction is only for a high-risk plan, or one that deletes or raises permissions. */
export function planNeedsConfirmation(
  risk: PlanRisk,
  flags?: PlanSafetyFlags,
): boolean {
  return risk === "high" || flags?.destructive === true || flags?.escalatesPrivileges === true;
}

/**
 * Same rule as the Run Plan button: non-empty command with no hidden or
 * direction-changing characters, and the typed folder name when required.
 */
export function planCanRun(input: {
  command: string;
  risk: PlanRisk;
  confirmed: boolean;
  flags?: PlanSafetyFlags;
}): boolean {
  const needsConfirmation = planNeedsConfirmation(input.risk, input.flags);
  return (
    input.command.trim().length > 0 &&
    !hasHiddenChars(input.command) &&
    (!needsConfirmation || input.confirmed)
  );
}

function uniqueWords(words: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const word of words) {
    if (!word || seen.has(word)) continue;
    seen.add(word);
    out.push(word);
  }
  return out;
}

type Props = {
  sessionId: string;
  intent: string;
  command: string;
  risk: PlanRisk;
  explanation: string;
  contextSources?: string[];
  plannerSource?: string;
  /** Planner flags beyond `risk`; written out in words, and used to gate a high-risk run. */
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
  const [typedConfirm, setTypedConfirm] = useState("");
  const [commandSync, setCommandSync] = useState(command);
  const [targetSync, setTargetSync] = useState(sessionId);
  const panelRef = useRef<HTMLDivElement>(null);
  const commandTextareaRef = useRef<HTMLTextAreaElement>(null);
  const setFocusZone = useFocusStore((s) => s.setFocusZone);

  // Reset the draft and the typed confirmation when a different command is shown,
  // before paint, so a previous confirmation cannot approve the new plan.
  if (command !== commandSync) {
    setCommandSync(command);
    setEditedCommand(command);
    setTypedConfirm("");
  }
  // A plan moved to another session (different cwd) needs a fresh confirmation too.
  if (sessionId !== targetSync) {
    setTargetSync(sessionId);
    setTypedConfirm("");
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
      confirmed: command ? confirmationMatches(typedConfirm, confirmationPhrase(target?.cwd)) : false,
    });
  }, [command, editedCommand, typedConfirm, target?.cwd, onRunGate]);

  if (!command) {
    return (
      <div className="plan-panel">
        <p className="muted">No plan yet.</p>
      </div>
    );
  }

  const phrase = confirmationPhrase(target?.cwd);
  const needsConfirmation = planNeedsConfirmation(risk, flags);
  const confirmed = needsConfirmation && confirmationMatches(typedConfirm, phrase);
  const hiddenWarning = describeHiddenChars(editedCommand);
  const canRun =
    !blockedReason &&
    planCanRun({
      command: editedCommand,
      risk,
      confirmed,
      flags,
    });
  const explained = explainCommand(editedCommand);
  const riskWords = riskInWords(risk, flags?.destructive === true, flags?.escalatesPrivileges === true);
  const flagWords = uniqueWords([
    ...(safetyFlags ?? []).map((code) => flagInWords(code)),
    flags?.destructive ? flagInWords("DESTRUCTIVE_OPERATION") : "",
    flags?.escalatesPrivileges ? flagInWords("PRIVILEGE_ESCALATION") : "",
    flags?.touchesNetwork ? flagInWords("NETWORK_ACCESS") : "",
    // touchesFiles is set when a command reads or writes. It is not a change,
    // so a listing must not be described as changing files.
  ]).filter((words) => words !== riskWords);

  return (
    <div
      ref={panelRef}
      className="plan-panel"
      tabIndex={0}
      aria-label="Command plan"
      onFocus={() => setFocusZone("plan")}
    >
      {plannerSource === "mock" && (
        <div className="plan-practice-notice muted">
          Practice plan — Ollama is not connected. This is not a real plan.
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
        <span className="plan-label">What this does</span>
        <p>{explained.sentence}</p>
        {explained.parts.length > 0 && (
          <ul className="plan-parts">
            {explained.parts.map((part, index) => (
              <li key={`${index}-${part.piece}`}>
                <code>{part.piece}</code> {part.meaning}
              </li>
            ))}
          </ul>
        )}
        {explained.touches.length > 0 && (
          <p>This will delete: {explained.touches.join(", ")}</p>
        )}
      </div>

      <div className="plan-section">
        <span className="plan-label">Risk</span>
        <p>{riskWords}</p>
        {flagWords.map((words) => (
          <p key={words}>{words}</p>
        ))}
      </div>

      {ambiguityFlags && ambiguityFlags.length > 0 && (
        <div className="plan-section plan-flags">
          <p>
            <span className="plan-label">Unclear</span> {ambiguityFlags.join("; ")}
          </p>
        </div>
      )}

      {explanation && explanation !== explained.sentence && (
        <div className="plan-section">
          <span className="plan-label">More detail</span>
          <p className="muted">{explanation}</p>
        </div>
      )}

      {needsConfirmation && (
        <label className="confirm-row" htmlFor="plan-confirm">
          Type {phrase} to run this
          <input
            id="plan-confirm"
            className="plan-confirm"
            value={typedConfirm}
            autoComplete="off"
            onChange={(e) => setTypedConfirm(e.target.value)}
          />
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
