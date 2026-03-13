import { useEffect } from "react";
import { useFocusStore } from "@commandui/state";
import type { ShortcutDef } from "../lib/shortcuts";
import { resolveShortcut } from "../lib/shortcuts";

/**
 * Attach a single window keydown listener that dispatches to the shortcut registry.
 * Reads current focus zone from Zustand to determine context.
 */
export function useShortcuts(shortcuts: ShortcutDef[]): void {
  const currentZone = useFocusStore((s) => s.currentZone);

  useEffect(() => {
    function handler(event: KeyboardEvent) {
      // Don't intercept when an input/textarea in a drawer or palette is focused
      // (the shortcut system handles this via zone guards)
      const match = resolveShortcut(shortcuts, event, currentZone);
      if (match) {
        event.preventDefault();
        event.stopPropagation();
        match.action();
      }
    }

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [shortcuts, currentZone]);
}
