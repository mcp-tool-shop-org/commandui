/** Missed-event bound for one workflow step. Not a general command budget. */
export const WORKFLOW_STEP_TIMEOUT_MS = 300_000;
export const WORKFLOW_STEP_POLL_MS = 100;

export type StepWaitResult<T> =
  | { ok: true; item: T }
  | { ok: false; reason: "timeout" | "aborted" };

/**
 * Poll until `readItem` returns a terminal item, the signal aborts, or the
 * timeout fires. The poll timer and the timeout are both cleared on every exit.
 */
export function waitForTerminalStatus<T>(
  readItem: () => T | undefined,
  isTerminal: (item: T) => boolean,
  signal: AbortSignal,
  timeoutMs = WORKFLOW_STEP_TIMEOUT_MS,
  pollMs = WORKFLOW_STEP_POLL_MS,
): Promise<StepWaitResult<T>> {
  return new Promise((resolve) => {
    let pollTimer: ReturnType<typeof setTimeout> | undefined;
    let timeoutTimer: ReturnType<typeof setTimeout> | undefined;
    let settled = false;

    const finish = (result: StepWaitResult<T>) => {
      if (settled) return;
      settled = true;
      if (pollTimer !== undefined) clearTimeout(pollTimer);
      if (timeoutTimer !== undefined) clearTimeout(timeoutTimer);
      signal.removeEventListener("abort", onAbort);
      resolve(result);
    };

    const onAbort = () => {
      finish({ ok: false, reason: "aborted" });
    };

    if (signal.aborted) {
      finish({ ok: false, reason: "aborted" });
      return;
    }

    const poll = () => {
      const item = readItem();
      if (item && isTerminal(item)) {
        finish({ ok: true, item });
        return;
      }
      pollTimer = setTimeout(poll, pollMs);
    };

    timeoutTimer = setTimeout(() => {
      finish({ ok: false, reason: "timeout" });
    }, timeoutMs);
    signal.addEventListener("abort", onAbort);
    pollTimer = setTimeout(poll, pollMs);
  });
}
