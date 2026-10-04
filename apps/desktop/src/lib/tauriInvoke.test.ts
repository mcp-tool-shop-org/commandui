import { afterEach, describe, expect, it, vi } from "vitest";
import { ERROR_CODES } from "./commandError";
import { tauriInvoke } from "./tauriInvoke";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
}));

describe("tauriInvoke", () => {
  afterEach(() => {
    invoke.mockReset();
    vi.useRealTimers();
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  function asTauri() {
    (window as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
  }

  it.each(ERROR_CODES)("rejects %s with the Rust message and that code", async (code) => {
    asTauri();
    invoke.mockRejectedValue({ code, message: `rust message for ${code}`, details: null });
    const error = await tauriInvoke("terminal_execute", {}).then(
      () => {
        throw new Error("expected a rejection");
      },
      (rejected: unknown) => rejected,
    );
    expect(error).toMatchObject({ code, message: `rust message for ${code}` });
    expect((error as Error).message).not.toContain("[object Object]");
    expect((error as Error).message).not.toContain("Command '");
  });

  it("says what to do when a call does not answer", async () => {
    asTauri();
    vi.useFakeTimers();
    invoke.mockReturnValue(new Promise(() => {}));
    const pending = tauriInvoke("session_list", {});
    const rejected = pending.then(
      () => {
        throw new Error("expected a rejection");
      },
      (error: unknown) => error,
    );
    await vi.advanceTimersByTimeAsync(15_000);
    const error = await rejected;
    expect(error).toMatchObject({
      code: "UNKNOWN_ERROR",
      message: "CommandUI did not answer in time. Try again. If it keeps happening, restart CommandUI.",
    });
    expect((error as Error).message).not.toMatch(/backend/i);
  });
});
