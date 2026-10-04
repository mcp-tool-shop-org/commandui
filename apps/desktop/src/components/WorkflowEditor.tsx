import { useEffect, useState } from "react";
import { useModalDialog } from "../lib/useModalDialog";

type Props = {
  initialLabel: string;
  initialSteps: string[];
  projectRoot?: string;
  /** create names a new list. edit changes one that is already saved. */
  mode?: "create" | "edit";
  onConfirm: (label: string, steps: string[]) => void;
  onCancel: () => void;
};

export function WorkflowEditor({
  initialLabel,
  initialSteps,
  mode = "create",
  onConfirm,
  onCancel,
}: Props) {
  const [label, setLabel] = useState(initialLabel);
  const [steps, setSteps] = useState<string[]>(initialSteps.length > 0 ? initialSteps : [""]);
  const [focusIndex, setFocusIndex] = useState<number | null>(null);
  const dialogRef = useModalDialog(true, onCancel);
  const creating = mode === "create";

  useEffect(() => {
    if (focusIndex == null) return;
    const index = focusIndex < 0 ? steps.length - 1 : focusIndex;
    const node = dialogRef.current?.querySelector<HTMLInputElement>(
      `[aria-label="Step ${index + 1}"]`,
    );
    node?.focus();
    setFocusIndex(null);
  }, [focusIndex, steps, dialogRef]);

  function updateStep(index: number, value: string) {
    setSteps((prev) => prev.map((s, i) => (i === index ? value : s)));
  }

  function moveUp(index: number) {
    if (index <= 0) return;
    setSteps((prev) => {
      const next = [...prev];
      [next[index - 1], next[index]] = [next[index], next[index - 1]];
      return next;
    });
  }

  function moveDown(index: number) {
    setSteps((prev) => {
      if (index >= prev.length - 1) return prev;
      const next = [...prev];
      [next[index], next[index + 1]] = [next[index + 1], next[index]];
      return next;
    });
  }

  function removeStep(index: number) {
    setSteps((prev) => (prev.length <= 1 ? prev : prev.filter((_, i) => i !== index)));
  }

  function addStep() {
    setSteps((prev) => [...prev, ""]);
    setFocusIndex(-1);
  }

  const canConfirm =
    label.trim() !== "" &&
    steps.length > 0 &&
    steps.every((s) => s.trim() !== "");

  function handleConfirm() {
    if (!canConfirm) return;
    onConfirm(
      label.trim(),
      steps.map((s) => s.trim()),
    );
  }

  return (
    <div className="palette-overlay" onClick={onCancel}>
      <div
        ref={dialogRef}
        className="palette-panel workflow-editor-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="workflow-editor-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="workflow-editor-header">
          <h3 id="workflow-editor-title">{creating ? "New workflow" : "Edit workflow"}</h3>
        </div>

        <div className="workflow-editor-body">
          <div className="workflow-editor-field">
            <label htmlFor="workflow-name">Name</label>
            <input
              id="workflow-name"
              className="workflow-editor-name"
              type="text"
              data-autofocus
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
          </div>

          <div className="workflow-editor-field">
            <label>Steps</label>
            {steps.map((step, i) => (
              <div key={i} className="workflow-editor-step">
                <span className="workflow-step-num">{i + 1}</span>
                <input
                  className="workflow-editor-step-input"
                  type="text"
                  aria-label={`Step ${i + 1}`}
                  value={step}
                  onChange={(e) => updateStep(i, e.target.value)}
                />
                <button
                  type="button"
                  disabled={i === 0}
                  onClick={() => moveUp(i)}
                  title="Move up"
                  aria-label={`Move step ${i + 1} up`}
                >
                  <span aria-hidden="true">↑</span>
                </button>
                <button
                  type="button"
                  disabled={i === steps.length - 1}
                  onClick={() => moveDown(i)}
                  title="Move down"
                  aria-label={`Move step ${i + 1} down`}
                >
                  <span aria-hidden="true">↓</span>
                </button>
                <button
                  type="button"
                  className="btn-danger"
                  disabled={steps.length <= 1}
                  onClick={() => removeStep(i)}
                  title="Remove step"
                  aria-label={`Remove step ${i + 1}`}
                >
                  <span aria-hidden="true">×</span>
                </button>
              </div>
            ))}
            <button type="button" className="workflow-editor-add" onClick={addStep}>
              Add step
            </button>
          </div>
        </div>

        <div className="workflow-editor-footer">
          <button type="button" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="workflow-editor-confirm"
            disabled={!canConfirm}
            onClick={handleConfirm}
          >
            {creating ? "Create workflow" : "Save changes"}
          </button>
        </div>
      </div>
    </div>
  );
}
