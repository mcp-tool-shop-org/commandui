import { mockInvoke } from "./mockBridge";
import { CommandError, commandErrorFrom } from "./commandError";

const TAURI_INVOKE_TIMEOUT_MS = 15000;

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function tauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriRuntime()) {
    return mockInvoke<T>(command, args);
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(
        new CommandError(
          "UNKNOWN_ERROR",
          "CommandUI did not answer in time. Try again. If it keeps happening, restart CommandUI.",
        ),
      );
    }, TAURI_INVOKE_TIMEOUT_MS);
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
