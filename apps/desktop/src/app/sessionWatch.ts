import type { SessionExecState } from "@commandui/api-contract";

export const APP_VERSION = "1.0.2";
/** How long a session may stay in "booting" before the UI offers Resync and Close. */
export const BOOT_STALL_MS = 20_000;
/** How long a typed command (or an interrupt) may hold a session before the UI explains the way out. */
export const FOREGROUND_STUCK_MS = 30_000;
/** Cadence of the one timer that watches every session's boot and foreground time. */
export const SESSION_WATCH_MS = 2_000;
export const SESSION_NOT_READY_MESSAGE =
  "The terminal is not ready yet. Wait for the session to finish starting, or resync it.";
export const SESSION_EXITED_MESSAGE =
  "The shell in this session has exited. Open a new session to continue.";
export const APP_NOTES_MAX = 200;
export const APP_NOTES_SHOWN = 6;

export const EXEC_STATES: readonly SessionExecState[] = [
  "booting",
  "ready",
  "running",
  "interrupting",
  "desynced",
  "userRunning",
];

export function simplifyText(text: string): string {
  const first = text.split(/[.!?]\s/)[0];
  return first + (first.endsWith(".") ? "" : ".");
}

export function detectOS(): "windows" | "macos" | "linux" {
  const p = navigator.platform.toLowerCase();
  if (p.includes("win")) return "windows";
  if (p.includes("mac")) return "macos";
  return "linux";
}

export function capNotes(list: string[], max = APP_NOTES_MAX): string[] {
  return list.length > max ? list.slice(-max) : list;
}

export function bootStallDue(elapsedMs: number, alreadyMarked: boolean): boolean {
  return elapsedMs >= BOOT_STALL_MS && !alreadyMarked;
}

export function foregroundStuckDue(elapsedMs: number, alreadyMarked: boolean): boolean {
  return elapsedMs >= FOREGROUND_STUCK_MS && !alreadyMarked;
}
