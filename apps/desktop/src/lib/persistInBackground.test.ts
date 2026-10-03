import { afterEach, describe, expect, it, vi } from "vitest";
import { persistInBackground } from "./persistInBackground";

describe("persistInBackground", () => {
  afterEach(() => vi.restoreAllMocks());

  // A rejection reaches console.warn only through the attached catch, so the
  // warning is the proof that it was handled. Vitest also fails the run on
  // any unhandled rejection.
  it("logs a rejected write with its label", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    persistInBackground(
      "history update",
      Promise.reject(new Error("history update: no history item with id h9")),
    );
    await new Promise((r) => setTimeout(r, 0));
    expect(warn).toHaveBeenCalledWith(
      "[persist] history update failed:",
      "history update: no history item with id h9",
    );
  });

  it("logs a non-Error rejection as-is", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    persistInBackground("workflow add", Promise.reject({ code: "DATABASE_ERROR" }));
    await new Promise((r) => setTimeout(r, 0));
    expect(warn).toHaveBeenCalledWith("[persist] workflow add failed:", { code: "DATABASE_ERROR" });
  });

  it("stays silent when the write succeeds", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    persistInBackground("settings update", Promise.resolve({ ok: true }));
    await new Promise((r) => setTimeout(r, 0));
    expect(warn).not.toHaveBeenCalled();
  });
});
