import { useEffect } from "react";
import { useFocusStore } from "@commandui/state";
import type { FocusZone } from "@commandui/state";

/**
 * Claim a focus zone when `active` is true.
 * Components call this to declare ownership of keyboard context.
 */
export function useFocusZone(zone: FocusZone, active: boolean): void {
  const setFocusZone = useFocusStore((s) => s.setFocusZone);

  useEffect(() => {
    if (active) {
      setFocusZone(zone);
    }
  }, [zone, active, setFocusZone]);
}
