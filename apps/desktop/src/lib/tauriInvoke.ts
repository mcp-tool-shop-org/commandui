import { mockInvoke } from "./mockBridge";

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

  const timeout = new Promise<never>((_, reject) => {
    setTimeout(() => {
      reject(
        new Error(
          `Command '${command}' timed out after ${TAURI_INVOKE_TIMEOUT_MS}ms. The backend may be unresponsive.`,
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
    const message = e instanceof Error ? e.message : String(e);
    console.error(`[tauriInvoke] Command '${command}' failed:`, {
      command,
      args,
      error: message,
      timestamp: new Date().toISOString(),
    });
    throw new Error(
      `Command '${command}' failed: ${message}. Please try again or reload the app if the problem persists.`,
    );
  }
}

export { isTauriRuntime };
