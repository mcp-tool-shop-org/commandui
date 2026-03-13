import { useEffect, useRef, useState } from "react";

type Props = {
  initialLabel: string;
  initialSteps: string[];
  projectRoot?: string;
  onConfirm: (label: string, steps: string[]) => void;
  onCancel: () => void;
};

export function WorkflowEditor({
  initialLabel,
  initialSteps,
  onConfirm,
  onCancel,
}: Props) {
  const [label, setLabel] = useState(initialLabel);
  const [steps, setSteps] = useState<string[]>(initialSteps);
  const nameRef = useRef<HTMLInputElement>(null);
  const submittedRef = useRef(false);

  useEffect(() => {
    requestAnimationFrame(() => nameRef.current?.focus());
  }, []);

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
    setSteps((prev) => prev.filter((_, i) => i !== index));
  }

  const canConfirm =
    label.trim() !== "" &&
    steps.length > 0 &&
    steps.every((s) => s.trim() !== "");

  function handleConfirm() {
    if (!canConfirm || submittedRef.current) return;
    submittedRef.current = true;
    onConfirm(
      label.trim(),
      steps.map((s) => s.trim()),
    );
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onCancel();
    }
  }

  return (
    <div className="palette-overlay" onClick={onCancel}>
      <div
        className="palette-panel workflow-editor-panel"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="workflow-editor-header">
          <h3>Edit Workflow</h3>
        </div>

        <div className="workflow-editor-body">
          <div className="workflow-editor-field">
            <label>Name</label>
            <input
              ref={nameRef}
              className="workflow-editor-name"
              type="text"
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
                  value={step}
                  onChange={(e) => updateStep(i, e.target.value)}
                />
                <button
                  type="button"
                  disabled={i === 0}
                  onClick={() => moveUp(i)}
                  title="Move up"
                >
                  ↑
                </button>
                <button
                  type="button"
                  disabled={i === steps.length - 1}
                  onClick={() => moveDown(i)}
                  title="Move down"
                >
                  ↓
                </button>
                <button
                  type="button"
                  className="btn-danger"
                  disabled={steps.length <= 1}
                  onClick={() => removeStep(i)}
                  title="Remove step"
                >
                  ×
                </button>
              </div>
            ))}
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
            Create Workflow
          </button>
        </div>
      </div>
    </div>
  );
}
