import { describe, expect, it } from "vitest";
import {
  CommandError,
  ERROR_CODES,
  commandErrorFrom,
  isNotFoundError,
  isSessionExitedError,
} from "./commandError";

describe("commandErrorFrom", () => {
  it.each(ERROR_CODES)("renders the Rust message for %s and keeps the code", (code) => {
    const error = commandErrorFrom({
      code,
      message: `rust message for ${code}`,
      details: null,
    });
    expect(error).toBeInstanceOf(CommandError);
    expect(error.code).toBe(code);
    expect(error.message).toBe(`rust message for ${code}`);
    expect(error.message).not.toContain("[object Object]");
    expect(error.message).not.toContain("Command '");
  });

  it.each(ERROR_CODES)("includes details for %s when Rust sent a sentence", (code) => {
    const error = commandErrorFrom({
      code,
      message: "the shell refused",
      details: "the pipe closed",
    });
    expect(error.code).toBe(code);
    expect(error.message).toBe("the shell refused the pipe closed");
    expect(error.details).toBe("the pipe closed");
  });

  it("uses a plain sentence when the payload has a code and no message", () => {
    const error = commandErrorFrom({ code: "DATABASE_ERROR" });
    expect(error.code).toBe("DATABASE_ERROR");
    expect(error.message).toBe("The save did not work. Try again.");
    expect(error.message).not.toContain("DATABASE_ERROR");
  });

  it("does not treat a readable message as a missing id unless the code says so", () => {
    const words = commandErrorFrom({
      code: "DATABASE_ERROR",
      message: "workflow delete: no workflow with id w1",
    });
    const missing = commandErrorFrom({
      code: "NOT_FOUND",
      message: "workflow delete: no workflow with id w1",
    });
    expect(isNotFoundError(words)).toBe(false);
    expect(isNotFoundError(missing)).toBe(true);
  });

  it("does not treat an exited sentence as an exited shell unless the code says so", () => {
    const words = commandErrorFrom({
      code: "EXECUTION_FAILED",
      message: "The shell in this session has exited; open a new session",
    });
    const exited = commandErrorFrom({
      code: "SESSION_EXITED",
      message: "The shell in this session has exited; open a new session",
    });
    expect(isSessionExitedError(words)).toBe(false);
    expect(isSessionExitedError(exited)).toBe(true);
  });
});
