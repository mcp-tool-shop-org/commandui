import { describe, it, expect } from "vitest";
import type {
  PlannerGeneratePlanRequest,
  TerminalExecuteRequest,
} from "./index";

describe("API Contract shapes", () => {
  it("PlannerGeneratePlanRequest compiles with expected fields", () => {
    const request: PlannerGeneratePlanRequest = {
      sessionId: "s1",
      userIntent: "show changed files",
      context: {
        sessionId: "s1",
        cwd: "/home/user/project",
        projectRoot: "/home/user/project",
        os: "linux",
        shell: "bash",
        recentCommands: ["git log"],
        memoryItems: [
          { kind: "preferred_package_manager", key: "pm", value: "pnpm", confidence: 0.9 },
        ],
        projectFacts: [],
      },
    };
    expect(request.sessionId).toBe("s1");
    expect(request.context.memoryItems.length).toBe(1);
  });

  it("TerminalExecuteRequest compiles with expected fields", () => {
    const request: TerminalExecuteRequest = {
      executionId: "e1",
      sessionId: "s1",
      command: "git status",
      source: "raw",
    };
    expect(request.command).toBe("git status");
  });
});
