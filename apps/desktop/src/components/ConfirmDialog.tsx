import { useEffect, useRef } from "react";

type Props = {
  title: string;
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
};

/**
 * In-app question. Escape and Cancel decline. The confirm button is the only
 * way to accept, so a key aimed at the rest of the window cannot approve it.
 */
export function ConfirmDialog({ title, message, confirmLabel, onConfirm, onCancel }: Props) {
  const dialogRef = useRef<HTMLElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const onConfirmRef = useRef(onConfirm);
  const onCancelRef = useRef(onCancel);
  onConfirmRef.current = onConfirm;
  onCancelRef.current = onCancel;

  useEffect(() => {
    const inside = (node: EventTarget | null) =>
      node instanceof Node && dialogRef.current !== null && dialogRef.current.contains(node);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onCancelRef.current();
        return;
      }
      if (!inside(e.target)) {
        e.preventDefault();
        e.stopPropagation();
        cancelRef.current?.focus();
      }
    };
    cancelRef.current?.focus();
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  return (
    <div className="confirm-overlay">
      <section
        ref={dialogRef}
        className="confirm-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
      >
        <h2 id="confirm-title">{title}</h2>
        <p className="confirm-message">{message}</p>
        <div className="confirm-actions">
          <button type="button" ref={cancelRef} onClick={() => onCancelRef.current()}>
            Cancel
          </button>
          <button type="button" onClick={() => onConfirmRef.current()}>
            {confirmLabel}
          </button>
        </div>
      </section>
    </div>
  );
}
