import { useModalDialog } from "../lib/useModalDialog";

const SHORTCUTS: Array<[string, string]> = [
  ["F1", "Open this help"],
  ["Ctrl+J", "Move focus to the command box. Tab stays in the terminal, so use this to leave it."],
  ["Ctrl+Shift+J", "Same move, when a terminal shortcut would otherwise keep Ctrl+J."],
  ["Ctrl+Shift+A", "Switch between Command and Ask"],
  ["Ctrl+K", "Open the command palette"],
  ["Ctrl+Shift+O", "Open the output view"],
  ["Ctrl+T", "New session"],
  ["Ctrl+Enter", "Approve the plan"],
  ["Escape", "Close the top dialog"],
];

type Props = {
  onClose: () => void;
};

export function HelpDialog({ onClose }: Props) {
  const ref = useModalDialog(true, onClose);
  return (
    <div className="welcome-overlay">
      <div
        ref={ref}
        className="welcome help-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="help-title"
      >
        <h2 id="help-title">Keyboard and reading help</h2>
        <p>
          Each command gets one result sentence. A screen reader hears that sentence once, in a
          polite status message. Errors that stop you are announced immediately.
        </p>
        <p>
          Output view lists each command, its result, and the output as plain text. Progress that
          redraws on one line is shown as its last state.
        </p>
        <ul className="welcome-keys" aria-label="Keyboard shortcuts">
          {SHORTCUTS.map(([keys, what]) => (
            <li key={keys}>
              <kbd>{keys}</kbd> {what}
            </li>
          ))}
        </ul>
        <button type="button" data-autofocus onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}
