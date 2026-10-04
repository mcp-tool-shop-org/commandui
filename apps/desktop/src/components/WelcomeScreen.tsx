import { useEffect, useRef } from "react";
import mark from "../assets/commandui-mark.png";

type Props = {
  /** Close the welcome and put the cursor in the command box. */
  onStart: () => void;
  showAtStartup: boolean;
  onShowAtStartupChange: (show: boolean) => void;
  /** The local model Ask uses, named so a user knows what to install. */
  plannerModel: string;
};

const SHORTCUTS: Array<[string, string]> = [
  ["Ctrl+K", "command palette"],
  ["Ctrl+J", "jump to the command box"],
  ["Ctrl+T", "new session"],
  ["Ctrl+Enter", "approve a plan"],
];

export function WelcomeScreen({ onStart, showAtStartup, onShowAtStartupChange, plannerModel }: Props) {
  const dialogRef = useRef<HTMLElement>(null);
  const startRef = useRef<HTMLButtonElement>(null);
  const onStartRef = useRef(onStart);
  onStartRef.current = onStart;

  // Modal while open. The terminal takes focus when its shell becomes ready,
  // which can be after this opens: focus is pulled back, and keys aimed
  // anywhere else are held so nothing is typed into the shell behind.
  // Enter (outside the checkbox) and Escape close it from anywhere.
  useEffect(() => {
    const inside = (node: EventTarget | null) =>
      node instanceof Node && dialogRef.current !== null && dialogRef.current.contains(node);
    // A dialog rendered later (help, a confirm) is on top. This one yields so it
    // does not close itself or pull focus out of that dialog.
    const isTop = () => {
      const root = dialogRef.current;
      if (!root) return false;
      const dialogs = document.querySelectorAll("[role='dialog'][aria-modal='true']");
      return dialogs.length === 0 || dialogs[dialogs.length - 1] === root;
    };
    const onKey = (e: KeyboardEvent) => {
      if (!isTop()) return;
      const onCheckbox = e.target instanceof HTMLInputElement && e.target.type === "checkbox";
      if (e.key === "Escape" || (e.key === "Enter" && !onCheckbox)) {
        e.preventDefault();
        e.stopPropagation();
        onStartRef.current();
        return;
      }
      if (!inside(e.target)) {
        e.preventDefault();
        e.stopPropagation();
        startRef.current?.focus();
      }
    };
    const onFocus = (e: FocusEvent) => {
      if (!isTop()) return;
      if (!inside(e.target)) startRef.current?.focus();
    };
    startRef.current?.focus();
    window.addEventListener("keydown", onKey, true);
    document.addEventListener("focusin", onFocus, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      document.removeEventListener("focusin", onFocus, true);
    };
  }, []);

  return (
    <div className="welcome-overlay">
      <section
        ref={dialogRef}
        className="welcome"
        role="dialog"
        aria-modal="true"
        aria-labelledby="welcome-title"
      >
        <header className="welcome-head">
          <img src={mark} alt="" width={64} height={64} className="welcome-mark" />
          <div>
            <h1 id="welcome-title">Welcome to CommandUI</h1>
            <p className="muted">
              A terminal that can plan commands for you, and never runs one without your OK.
            </p>
          </div>
        </header>

        <ol className="welcome-steps">
          <li>
            <h2>Run commands</h2>
            <p>
              Type in the box at the bottom, as in any terminal. <strong>Command</strong> mode runs
              exactly what you type.
            </p>
          </li>
          <li>
            <h2>Ask in plain words</h2>
            <p>
              Switch to <strong>Ask</strong> and describe the task, like “find the biggest log
              files”. CommandUI drafts the command with a risk level and an explanation. Nothing
              runs until you approve it.
            </p>
          </li>
          <li>
            <h2>Keep what works</h2>
            <p>
              <strong>History</strong>, <strong>Workflows</strong> and <strong>Memory</strong> (top
              right) keep past commands, turn repeated steps into one-click workflows, and remember
              facts about your project.
            </p>
          </li>
        </ol>

        <p className="welcome-note muted">
          Ask uses a local Ollama model (<code>{plannerModel}</code>) on this PC, so your requests
          stay on your machine. Without it, Ask falls back to a basic built-in planner, marked{" "}
          <em>mock</em>.
        </p>

        <ul className="welcome-keys" aria-label="Keyboard shortcuts">
          {SHORTCUTS.map(([keys, what]) => (
            <li key={keys}>
              <kbd>{keys}</kbd> {what}
            </li>
          ))}
        </ul>

        <footer className="welcome-foot">
          <label>
            <input
              type="checkbox"
              checked={showAtStartup}
              onChange={(e) => onShowAtStartupChange(e.target.checked)}
            />{" "}
            Show this when CommandUI opens
          </label>
          <button ref={startRef} type="button" className="primary" onClick={onStart}>
            Get started
          </button>
        </footer>
      </section>
    </div>
  );
}
