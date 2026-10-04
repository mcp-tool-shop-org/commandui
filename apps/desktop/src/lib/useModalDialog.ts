import { useEffect, useRef } from "react";

const FOCUSABLE = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

function focusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => !el.hasAttribute("disabled") && el.tabIndex !== -1,
  );
}

/**
 * Makes an open panel a modal dialog: focus moves in, Tab stays inside,
 * Escape closes it, and focus returns to whatever opened it.
 * A dialog rendered later in the document is treated as on top, so a confirm
 * can sit over a drawer without the drawer stealing Escape or focus.
 */
export function useModalDialog(open: boolean, onClose: () => void) {
  const ref = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const root = ref.current;

    const isTop = () => {
      if (!root) return false;
      const dialogs = document.querySelectorAll("[role='dialog'][aria-modal='true']");
      return dialogs.length === 0 || dialogs[dialogs.length - 1] === root;
    };

    const focusFirst = () => {
      if (!root) return;
      const preferred = root.querySelector<HTMLElement>("[data-autofocus]");
      (preferred ?? focusable(root)[0])?.focus();
    };

    const onKey = (event: KeyboardEvent) => {
      if (!root || !isTop()) return;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const items = focusable(root);
      if (items.length === 0) {
        event.preventDefault();
        return;
      }
      const index = items.indexOf(document.activeElement as HTMLElement);
      const leavingForward = !event.shiftKey && (index === -1 || index === items.length - 1);
      const leavingBackward = event.shiftKey && index <= 0;
      if (leavingForward || leavingBackward) {
        event.preventDefault();
        items[leavingBackward ? items.length - 1 : 0].focus();
      }
    };

    const onFocus = (event: FocusEvent) => {
      if (!root || !isTop()) return;
      if (event.target instanceof Node && root.contains(event.target)) return;
      focusFirst();
    };

    focusFirst();
    window.addEventListener("keydown", onKey, true);
    document.addEventListener("focusin", onFocus, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      document.removeEventListener("focusin", onFocus, true);
      previous?.focus();
    };
  }, [open]);

  return ref;
}
