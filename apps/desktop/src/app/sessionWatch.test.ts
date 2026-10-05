import { afterEach, describe, expect, it, vi } from "vitest";
import {
  APP_NOTES_MAX,
  BOOT_STALL_MS,
  FOREGROUND_STUCK_MS,
  bootStallDue,
  capNotes,
  detectOS,
  foregroundStuckDue,
  simplifyText,
} from "./sessionWatch";

describe("session watch helpers", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("keeps the first sentence and always ends it with a period", () => {
    expect(simplifyText("Lists files. Then it stops.")).toBe("Lists files.");
    expect(simplifyText("Ready? Wait.")).toBe("Ready.");
    expect(simplifyText("Done!")).toBe("Done!.");
    expect(simplifyText("already.")).toBe("already.");
  });

  it("reads the platform the browser reports", () => {
    vi.stubGlobal("navigator", { platform: "Win32" });
    expect(detectOS()).toBe("windows");
    vi.stubGlobal("navigator", { platform: "MacIntel" });
    expect(detectOS()).toBe("macos");
    vi.stubGlobal("navigator", { platform: "Linux x86_64" });
    expect(detectOS()).toBe("linux");
  });

  it("keeps the newest notes once the list is over the cap", () => {
    const short = ["a", "b"];
    expect(capNotes(short)).toBe(short);
    const long = Array.from({ length: APP_NOTES_MAX + 3 }, (_, i) => `n${i}`);
    const capped = capNotes(long);
    expect(capped).toHaveLength(APP_NOTES_MAX);
    expect(capped[0]).toBe("n3");
    expect(capped.at(-1)).toBe(`n${APP_NOTES_MAX + 2}`);
  });

  it("offers a way out once, and only after the wait has elapsed", () => {
    expect(bootStallDue(BOOT_STALL_MS - 1, false)).toBe(false);
    expect(bootStallDue(BOOT_STALL_MS, false)).toBe(true);
    expect(bootStallDue(BOOT_STALL_MS + 1, true)).toBe(false);
    expect(foregroundStuckDue(FOREGROUND_STUCK_MS - 1, false)).toBe(false);
    expect(foregroundStuckDue(FOREGROUND_STUCK_MS, false)).toBe(true);
    expect(foregroundStuckDue(FOREGROUND_STUCK_MS, true)).toBe(false);
  });
});
