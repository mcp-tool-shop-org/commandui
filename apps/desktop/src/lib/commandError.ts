import type { BackendError } from "@commandui/api-contract";

export type ErrorCode = BackendError["code"];

/** Plain sentence when a rejection arrives with a code and no message. */
const FALLBACK = {
  SESSION_NOT_FOUND: "That session is not open. Open a new session and try again.",
  SESSION_DISCONNECTED: "The session disconnected. Open a new session and try again.",
  SESSION_EXITED: "The shell in this session has exited. Open a new session to continue.",
  EXECUTION_FAILED: "The command did not run. Check the terminal and try again.",
  PLANNER_FAILED: "A plan could not be made. Try again in a moment.",
  VALIDATION_FAILED: "That request was not accepted. Check it and try again.",
  DATABASE_ERROR: "The save did not work. Try again.",
  NOT_FOUND: "That item is no longer there.",
  NOT_IMPLEMENTED: "That action is not available yet.",
  UNKNOWN_ERROR: "Something went wrong. Try again.",
} as const satisfies Record<ErrorCode, string>;

export const ERROR_CODES = Object.keys(FALLBACK) as ErrorCode[];

/**
 * A rejected backend call. `message` is the text to show. `code` is for
 * matching. Callers do not match on the words of the message.
 */
export class CommandError extends Error {
  readonly code: ErrorCode;
  readonly details?: string;

  constructor(code: ErrorCode, message: string, details?: string) {
    super(message);
    this.name = "CommandError";
    this.code = code;
    this.details = details;
  }
}

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && Object.prototype.hasOwnProperty.call(FALLBACK, value);
}

function readable(code: ErrorCode, message: unknown, details: unknown): { message: string; details?: string } {
  const extra = typeof details === "string" && details.trim() ? details.trim() : undefined;
  const base =
    typeof message === "string" && message.trim() ? message.trim() : FALLBACK[code];
  const shown = extra && !base.includes(extra) ? `${base} ${extra}` : base;
  return { message: shown, details: extra };
}

/** Turn a rejection into an error whose message is safe to show. */
export function commandErrorFrom(value: unknown): CommandError {
  if (value instanceof CommandError) return value;

  if (typeof value === "object" && value !== null) {
    const code = (value as { code?: unknown }).code;
    if (isErrorCode(code)) {
      const payload = value as { message?: unknown; details?: unknown };
      const rendered = readable(code, payload.message, payload.details);
      return new CommandError(code, rendered.message, rendered.details);
    }
  }

  if (value instanceof Error && value.message.trim()) {
    return new CommandError("UNKNOWN_ERROR", value.message);
  }
  if (typeof value === "string" && value.trim()) {
    return new CommandError("UNKNOWN_ERROR", value.trim());
  }
  return new CommandError("UNKNOWN_ERROR", FALLBACK.UNKNOWN_ERROR);
}

export function errorText(value: unknown): string {
  return commandErrorFrom(value).message;
}

/** The store does not hold that id, or the suggestion is no longer pending. */
export function isNotFoundError(value: unknown): boolean {
  return value instanceof CommandError && value.code === "NOT_FOUND";
}

/** The shell for this session is gone. */
export function isSessionExitedError(value: unknown): boolean {
  return value instanceof CommandError && value.code === "SESSION_EXITED";
}
