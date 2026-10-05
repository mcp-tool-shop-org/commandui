import { useEffect, useRef, useState } from "react";
import { useModalDialog } from "../lib/useModalDialog";

export type PaletteAction = {
  id: string;
  label: string;
  shortcut?: string;
  action: () => void;
};

type Props = {
  isOpen: boolean;
  onClose: () => void;
  actions: PaletteAction[];
};

export function CommandPalette({ isOpen, onClose, actions }: Props) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const dialogRef = useModalDialog(isOpen, onClose);

  const filtered = query
    ? actions.filter((a) =>
        a.label.toLowerCase().includes(query.toLowerCase()),
      )
    : actions;

  // Reset state on open
  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelectedIndex(0);
      // Focus after paint so overlay is mounted
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  // Clamp selection when filter changes
  useEffect(() => {
    setSelectedIndex((i) => Math.min(i, Math.max(0, filtered.length - 1)));
  }, [filtered.length]);

  if (!isOpen) return null;

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (filtered.length === 0) return;
      setSelectedIndex((i) => (Math.min(i, filtered.length - 1) + 1) % filtered.length);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (filtered.length === 0) return;
      setSelectedIndex(
        (i) => (Math.min(i, filtered.length - 1) - 1 + filtered.length) % filtered.length,
      );
      return;
    }
    if (e.key === "Enter" && filtered.length > 0) {
      e.preventDefault();
      filtered[Math.min(Math.max(0, selectedIndex), filtered.length - 1)].action();
      onClose();
      return;
    }
  }

  return (
    <div className="palette-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className="palette-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <input
          ref={inputRef}
          className="palette-search"
          type="text"
          aria-label="Search commands"
          data-autofocus
          placeholder="Type a command…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="palette-list" role="listbox" aria-label="Matching commands">
          {filtered.map((action, i) => (
            <div
              key={action.id}
              className={`palette-item${i === selectedIndex ? " palette-item-selected" : ""}`}
              role="option"
              aria-selected={i === selectedIndex}
              onMouseEnter={() => setSelectedIndex(i)}
              onClick={() => {
                action.action();
                onClose();
              }}
            >
              <span className="palette-item-label">{action.label}</span>
              {action.shortcut && (
                <span className="palette-item-shortcut">{action.shortcut}</span>
              )}
            </div>
          ))}
          {filtered.length === 0 && (
            <div className="palette-empty">No matching commands</div>
          )}
        </div>
      </div>
    </div>
  );
}
