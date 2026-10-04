import { afterEach, describe, expect, it, vi } from "vitest";
import { persistInBackground } from "./persistInBackground";

describe("persistInBackground", () => {
  afterEach(() => vi.restoreAllMocks());

  // A rejection reaches console.warn only through the attached catch, so the
  // warning is the proof that it was handled. Vitest also fails the run on
  // any unhandled rejection.
  it("logs a rejected write with its label", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const dispatchEventSpy = vi.spyOn(window, "dispatchEvent").mockImplementation(() => true);
    persistInBackground(
      "history update",
      Promise.reject(new Error("history update: no history item with id h9")),
    );
    await new Promise((r) => setTimeout(r, 0));
    expect(warn).toHaveBeenCalledWith(
      "[persist] history update failed:",
      "history update: no history item with id h9",
    );
    expect(dispatchEventSpy).toHaveBeenCalled();
    const event = dispatchEventSpy.mock.calls[0][0] as CustomEvent;
    expect(event.type).toBe("commandui:persist-failed");
    expect(event.detail).toMatchObject({
      what: "history update",
      message: "history update: no history item with id h9",
    });
  });

  it("logs a non-Error rejection as-is", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const dispatchEventSpy = vi.spyOn(window, "dispatchEvent").mockImplementation(() => true);
    persistInBackground("workflow add", Promise.reject({ code: "DATABASE_ERROR" }));
    await new Promise((r) => setTimeout(r, 0));
    expect(warn).toHaveBeenCalledWith("[persist] workflow add failed:", { code: "DATABASE_ERROR" });
    expect(dispatchEventSpy).toHaveBeenCalled();
    const event = dispatchEventSpy.mock.calls[0][0] as CustomEvent;
    expect(event.type).toBe("commandui:persist-failed");
    expect(event.detail).toMatchObject({
      what: "workflow add",
      message: "[object Object]",
    });
  });

  it("stays silent when the write succeeds", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const dispatchEventSpy = vi.spyOn(window, "dispatchEvent").mockImplementation(() => true);
    persistInBackground("settings update", Promise.resolve({ ok: true }));
    await new Promise((r) => setTimeout(r, 0));
    expect(warn).not.toHaveBeenCalled();
    expect(dispatchEventSpy).not.toHaveBeenCalled();
  });
});
