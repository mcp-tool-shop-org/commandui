import { describe, it, expect, expectTypeOf } from "vitest";
import * as api from "./index";
import type {
  PlannerContext,
  PlannerGeneratePlanRequest,
  TerminalExecuteRequest,
} from "./index";

// These are compile-time checks. vitest erases expectTypeOf calls and the
// expect-error directives at runtime, so `vitest run` cannot fail on contract drift.
// The enforcement is the `tsc --noEmit` typecheck step (pnpm typecheck).

describe("API Contract shapes", () => {
  it("loads the barrel, which re-exports types and no runtime values", () => {
    expect(Object.keys(api)).toEqual([]);
  });

  it("PlannerGeneratePlanRequest keeps its required keys and unions", () => {
    expectTypeOf<PlannerGeneratePlanRequest>().toHaveProperty("sessionId");
    expectTypeOf<PlannerGeneratePlanRequest>().toHaveProperty("userIntent");
    expectTypeOf<PlannerGeneratePlanRequest>().toHaveProperty("context");
    expectTypeOf<PlannerGeneratePlanRequest["sessionId"]>().toEqualTypeOf<string>();
    expectTypeOf<PlannerContext["os"]>().toEqualTypeOf<
      "windows" | "macos" | "linux"
    >();
    expectTypeOf<PlannerContext["recentCommands"]>().toEqualTypeOf<string[]>();

    const context: PlannerContext = {
      sessionId: "s1",
      cwd: "/home/user/project",
      os: "linux",
      shell: "bash",
      recentCommands: [],
      memoryItems: [],
      projectFacts: [],
    };
    const request: PlannerGeneratePlanRequest = {
      sessionId: "s1",
      userIntent: "show changed files",
      context,
    };

    // @ts-expect-error missing required userIntent must be rejected
    const missing: PlannerGeneratePlanRequest = { sessionId: "s1", context };
    // @ts-expect-error os outside the union must be rejected
    const badOs: PlannerContext = { ...context, os: "beos" };
    // @ts-expect-error unknown extra keys must be rejected
    const extra: PlannerGeneratePlanRequest = { ...request, bogus: true };

    expect([request, missing, badOs, extra]).toHaveLength(4);
  });

  it("TerminalExecuteRequest keeps its required keys and source union", () => {
    expectTypeOf<TerminalExecuteRequest["source"]>().toEqualTypeOf<
      "raw" | "semantic"
    >();
    expectTypeOf<TerminalExecuteRequest["executionId"]>().toEqualTypeOf<string>();
    expectTypeOf<TerminalExecuteRequest["command"]>().toEqualTypeOf<string>();

    const ok: TerminalExecuteRequest = {
      executionId: "e1",
      sessionId: "s1",
      command: "git status",
      source: "raw",
    };
    // @ts-expect-error source outside the union must be rejected
    const badSource: TerminalExecuteRequest = { ...ok, source: "manual" };
    // @ts-expect-error missing required command must be rejected
    const noCommand: TerminalExecuteRequest = {
      executionId: "e1",
      sessionId: "s1",
      source: "raw",
    };
    expect([ok, badSource, noCommand]).toHaveLength(3);
  });
});
