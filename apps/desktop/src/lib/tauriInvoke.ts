import { mockInvoke } from "./mockBridge";
import { CommandError, commandErrorFrom } from "./commandError";

const TAURI_INVOKE_TIMEOUT_MS = 15000;
const TIMEOUT_MESSAGE =
  "CommandUI did not answer in time. Try again. If it keeps happening, restart CommandUI.";

export type InvokeOptions = {
  /** How long to wait. Most calls answer at once; a plan can wait for a model to load. */
  timeoutMs?: number;
  /** What to tell the user when the wait runs out. */
  timeoutMessage?: string;
};

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function tauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  options: InvokeOptions = {},
): Promise<T> {
  if (!isTauriRuntime()) {
    return mockInvoke<T>(command, args);
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(
        new CommandError("UNKNOWN_ERROR", options.timeoutMessage ?? TIMEOUT_MESSAGE),
      );
    }, options.timeoutMs ?? TAURI_INVOKE_TIMEOUT_MS);
  });

  try {
    const invokePromise = import("@tauri-apps/api/core").then(({ invoke }) =>
      invoke<T>(command, args),
    );
    return await Promise.race([invokePromise, timeout]);
  } catch (e: unknown) {
    const error = commandErrorFrom(e);
    console.error("[tauriInvoke] rejected", {
      command,
      args,
      code: error.code,
      message: error.message,
      timestamp: new Date().toISOString(),
    });
    throw error;
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

export { isTauriRuntime };
