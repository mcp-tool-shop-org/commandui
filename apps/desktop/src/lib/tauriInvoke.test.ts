import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { resetMockBridge } from "./mockBridge";
import { isTauriRuntime, tauriInvoke } from "./tauriInvoke";

const invokeMock = vi.mocked(invoke);

describe("tauriInvoke", () => {
  afterEach(() => {
    resetMockBridge();
    delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    vi.useRealTimers();
    invokeMock.mockReset();
  });

  it("uses the mock bridge when the window is not running inside Tauri", async () => {
    expect(isTauriRuntime()).toBe(false);
    const listed = await tauriInvoke<{ sessions: unknown[] }>("session_list");
    expect(listed.sessions).toEqual([]);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("wraps a backend failure with the command name", async () => {
    vi.useFakeTimers();
    (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
    invokeMock.mockRejectedValue(new Error("disk full"));
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(tauriInvoke("session_list", { request: {} })).rejects.toThrow(
      "Command 'session_list' failed: disk full.",
    );
    expect(error).toHaveBeenCalled();
    error.mockRestore();
    vi.clearAllTimers();
  });

  it("reports a command that never returns", async () => {
    vi.useFakeTimers();
    (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
    invokeMock.mockReturnValue(new Promise(() => {}));
    const pending = tauriInvoke("session_list");
    const failed = expect(pending).rejects.toThrow("Command 'session_list' timed out after 15000ms.");
    await vi.advanceTimersByTimeAsync(15_000);
    await failed;
  });

  it("stringifies a rejection that is not an Error", async () => {
    vi.useFakeTimers();
    (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
    invokeMock.mockRejectedValue("nope");
    vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(tauriInvoke("session_list")).rejects.toThrow("Command 'session_list' failed: nope.");
    vi.clearAllTimers();
  });
});
