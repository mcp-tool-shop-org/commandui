import type { SessionExecState } from "@commandui/api-contract";

export const SESSION_BUSY_MESSAGE = "A command is already running in this session.";
export const SESSION_USER_RUNNING_MESSAGE =
  "A command you typed is still running in this session. Wait for the prompt, or type exit in the terminal if it is a nested shell or ssh session.";

/** A command holds the shell: one the app ran, one being interrupted, or one the user typed. */
export function execIsBusy(exec: SessionExecState | undefined): boolean {
  return exec === "running" || exec === "interrupting" || exec === "userRunning";
}

/**
 * Only a command the app started owns the "running" badge. A hand-typed command emits no
 * ExecutionFinished, so a badge raised for it would never be lowered.
 */
export function execOwnsBadge(exec: SessionExecState | undefined): boolean {
  return exec === "running" || exec === "interrupting";
}

export function busyMessageFor(exec: SessionExecState | undefined): string {
  return exec === "userRunning" ? SESSION_USER_RUNNING_MESSAGE : SESSION_BUSY_MESSAGE;
}

/** The composer takes input only when the shell is at a prompt and alive. */
export function composerDisabled(exec: SessionExecState, exited: boolean): boolean {
  return exec !== "ready" || exited;
}

/** A state a session can sit in for long without being stuck. */
export function execIsForeground(exec: SessionExecState | undefined): boolean {
  return exec === "userRunning" || exec === "interrupting";
}
