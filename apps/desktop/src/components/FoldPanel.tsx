import { useState, type ReactNode } from "react";

type Props = {
  hideLabel: string;
  showLabel: string;
  children: ReactNode;
};

/**
 * Starts open. At large text these panels use the room the terminal needs,
 * and hiding one is a choice the person makes.
 */
export function FoldPanel({ hideLabel, showLabel, children }: Props) {
  const [open, setOpen] = useState(true);
  return (
    <div className="fold">
      <button
        type="button"
        className="fold-toggle"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        {open ? hideLabel : showLabel}
      </button>
      <div className="fold-body" hidden={!open}>
        {children}
      </div>
    </div>
  );
}
