/**
 * Persistence writes the UI does not wait on (history rows, plans, workflows,
 * settings). The backend reports an unknown id or a database error as a
 * rejection; a bare `void write()` let that escape as an unhandled rejection.
 * Here a failure is logged with what was being written and goes no further.
 */
export function persistInBackground(what: string, write: Promise<unknown>): void {
  write.catch((e: unknown) => {
    console.warn(`[persist] ${what} failed:`, e instanceof Error ? e.message : e);
  });
}
