import { errorText } from "./commandError";

/**
 * Persistence writes the UI does not wait on (history rows, plans, workflows,
 * settings). A rejection is logged with what was being written and surfaced
 * to the user via a dispatched event so the shell can show a banner.
 */

export function persistInBackground(what: string, write: Promise<unknown>): void {
  write.catch((e: unknown) => {
    const message = errorText(e);
    console.warn(`[persist] ${what} failed:`, message);
    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("commandui:persist-failed", {
          detail: { what, message, timestamp: new Date().toISOString() },
        }),
      );
    }
  });
}
