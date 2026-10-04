import { describe, expect, it } from "vitest";
import type { SessionExecState } from "@commandui/api-contract";
import {
  SESSION_BUSY_MESSAGE,
  SESSION_USER_RUNNING_MESSAGE,
  busyMessageFor,
  composerDisabled,
  execIsBusy,
  execIsForeground,
  execOwnsBadge,
} from "./execState";

const states: Array<SessionExecState | undefined> = [
  undefined,
  "booting",
  "ready",
  "running",
  "interrupting",
  "desynced",
  "userRunning",
];

describe("execState", () => {
  it("treats an app command, an interrupt, and a typed command as busy", () => {
    for (const exec of states) {
      const busy = exec === "running" || exec === "interrupting" || exec === "userRunning";
      expect(execIsBusy(exec), String(exec)).toBe(busy);
    }
  });

  it("gives the running badge only to a command the app started", () => {
    for (const exec of states) {
      const owns = exec === "running" || exec === "interrupting";
      expect(execOwnsBadge(exec), String(exec)).toBe(owns);
    }
  });

  it("uses the typed-command message only for userRunning", () => {
    expect(busyMessageFor("userRunning")).toBe(SESSION_USER_RUNNING_MESSAGE);
    for (const exec of states.filter((exec) => exec !== "userRunning")) {
      expect(busyMessageFor(exec), String(exec)).toBe(SESSION_BUSY_MESSAGE);
    }
  });

  it("enables the composer only at a live prompt", () => {
    expect(composerDisabled("ready", false)).toBe(false);
    expect(composerDisabled("ready", true)).toBe(true);
    for (const exec of ["booting", "running", "interrupting", "desynced", "userRunning"] as const) {
      expect(composerDisabled(exec, false), exec).toBe(true);
    }
  });

  it("treats a typed command and an interrupt as foreground states", () => {
    for (const exec of states) {
      const foreground = exec === "userRunning" || exec === "interrupting";
      expect(execIsForeground(exec), String(exec)).toBe(foreground);
    }
  });
});
