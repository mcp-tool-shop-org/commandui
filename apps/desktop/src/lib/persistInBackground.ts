/**
 * Persistence writes the UI does not wait on (history rows, plans, workflows,
 * settings). The backend reports an unknown id or a database error as a
 * rejection; a bare `void write()` let that escape as an unhandled rejection.
 * Here a failure is logged with what was being written and surfaced to the user
 * via a dispatched event so the shell can show a banner.
 */
export function persistInBackground(what: string, write: Promise<unknown>): void {
  write.catch((e: unknown) => {
    console.warn(`[persist] ${what} failed:`, e instanceof Error ? e.message : e);
    const message = e instanceof Error ? e.message : String(e);
    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("commandui:persist-failed", {
          detail: { what, message, timestamp: new Date().toISOString() },
        }),
      );
    }
  });
}
