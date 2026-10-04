import { describe, expect, it } from "vitest";
import {
  askFixPrompt,
  collapseRedraws,
  countOutputLines,
  describeResult,
  historyStatusLabel,
  looksLikeRequest,
  resultText,
} from "./commandResult";

describe("looksLikeRequest", () => {
  it("treats a sentence in Command mode as a request", () => {
    expect(looksLikeRequest("list the files")).toBe(true);
    expect(looksLikeRequest("how do I see the folder")).toBe(true);
    expect(looksLikeRequest("please show the notes")).toBe(true);
    expect(looksLikeRequest("what is in this folder?")).toBe(true);
    expect(looksLikeRequest("copy the notes to the archive")).toBe(true);
  });

  it("leaves real commands alone", () => {
    expect(looksLikeRequest("git status")).toBe(false);
    expect(looksLikeRequest("echo hello")).toBe(false);
    expect(looksLikeRequest("dir")).toBe(false);
    expect(looksLikeRequest("npm test -- --run")).toBe(false);
    expect(looksLikeRequest("C:\\Work\\notes")).toBe(false);
    expect(looksLikeRequest("./scripts/build.sh")).toBe(false);
    expect(looksLikeRequest("")).toBe(false);
  });
});

describe("describeResult", () => {
  it("says how many lines finished", () => {
    const result = describeResult({ phase: "success", outputLines: 24 });
    expect(result.headline).toBe("Finished. 24 lines of output.");
    expect(result.actions).toEqual(["show-output"]);
    expect(describeResult({ phase: "success", outputLines: 1 }).headline).toBe(
      "Finished. 1 line of output.",
    );
  });

  it("says a command is running and offers Stop", () => {
    const result = describeResult({ phase: "running" });
    expect(result.headline).toBe("Running… (Stop)");
    expect(result.actions).toEqual(["stop"]);
    expect(result.announce).toBe(false);
  });

  it("names a real exit code and offers the three actions", () => {
    const result = describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: "nope" });
    expect(result.headline).toBe("Did not work (exit code 1)");
    expect(result.reason).toBe("The command finished with an error.");
    expect(result.actions).toEqual(["show-output", "ask-fix", "run-again"]);
  });

  it("explains command not found from the exit code and from the output", () => {
    expect(describeResult({ phase: "failure", exitCode: 127, exitKnown: true }).reason).toBe(
      "The shell could not find that command.",
    );
    expect(
      describeResult({
        phase: "failure",
        exitCode: 1,
        exitKnown: true,
        outputText: "'foo' is not recognized as an internal or external command",
      }).reason,
    ).toBe("The shell could not find that command.");
    expect(
      describeResult({
        phase: "failure",
        exitCode: 127,
        exitKnown: true,
        outputText: "bash: foo: command not found",
      }).headline,
    ).toBe("Did not work (exit code 127)");
  });

  it("explains access denied", () => {
    expect(describeResult({ phase: "failure", exitCode: 5, exitKnown: true }).reason).toBe(
      "CommandUI was not allowed to do that.",
    );
    expect(
      describeResult({
        phase: "failure",
        exitCode: 1,
        exitKnown: true,
        outputText: "Access is denied.",
      }).reason,
    ).toBe("CommandUI was not allowed to do that.");
  });

  it("explains a missing path", () => {
    expect(
      describeResult({
        phase: "failure",
        exitCode: 1,
        exitKnown: true,
        outputText: "The system cannot find the path specified.",
      }).reason,
    ).toBe("A file or folder in that command is not there.");
    expect(
      describeResult({
        phase: "failure",
        exitCode: 1,
        exitKnown: true,
        outputText: "No such file or directory",
      }).reason,
    ).toBe("A file or folder in that command is not there.");
  });

  it("does not invent exit code 1 when the code is unknown", () => {
    const result = describeResult({
      phase: "unknown",
      exitCode: 1,
      exitKnown: false,
      reason: "exit_unknown",
    });
    expect(result.headline).toBe("CommandUI could not tell whether this worked");
    expect(resultText(result)).not.toMatch(/exit code \d/i);
    expect(result.actions).toEqual(["show-output", "ask-fix", "run-again"]);
  });

  it("says the shell exited without quoting an exit code", () => {
    const result = describeResult({
      phase: "failure",
      exitCode: 1,
      exitKnown: false,
      reason: "shell_exited",
    });
    expect(result.headline).toBe("The shell exited.");
    expect(result.next).toBe("Open a new session to continue.");
    expect(resultText(result).toLowerCase()).not.toContain("exit code");
  });

  it("says the terminal did not accept the input", () => {
    const result = describeResult({
      phase: "failure",
      exitCode: 1,
      exitKnown: false,
      reason: "input_not_accepted",
    });
    expect(result.reason).toBe("The terminal did not accept that input.");
    expect(resultText(result).toLowerCase()).not.toContain("exit code");
    expect(result.actions).toEqual(["run-again"]);
  });

  it("offers to ask instead of running a sentence", () => {
    const result = describeResult({ phase: "request", command: "list the files" });
    expect(result.headline).toBe("This looks like a request. Ask CommandUI instead?");
    expect(result.actions).toEqual(["ask-instead"]);
  });

  it("never uses the word FAILURE as the whole result", () => {
    const cases = [
      describeResult({ phase: "success", outputLines: 0 }),
      describeResult({ phase: "running" }),
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true }),
      describeResult({ phase: "failure", exitCode: 127, exitKnown: true }),
      describeResult({ phase: "failure", exitCode: 5, exitKnown: true }),
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: "cannot find the path" }),
      describeResult({ phase: "unknown", exitCode: 1, exitKnown: false, reason: "exit_unknown" }),
      describeResult({ phase: "failure", exitCode: 1, exitKnown: false, reason: "shell_exited" }),
      describeResult({ phase: "failure", exitCode: 1, exitKnown: false, reason: "input_not_accepted" }),
      describeResult({ phase: "interrupted" }),
      describeResult({ phase: "request" }),
    ];
    for (const result of cases) {
      const text = resultText(result);
      expect(text).not.toMatch(/\bFAILURE\b/);
      expect(text.trim().toLowerCase()).not.toBe("failure");
      expect(result.headline.trim().toLowerCase()).not.toBe("failure");
    }
  });
});

describe("askFixPrompt", () => {
  it("sends the command, the real exit code, and the last lines", () => {
    const output = Array.from({ length: 50 }, (_, i) => `line ${i}`).join("\n");
    const prompt = askFixPrompt({
      command: "git status",
      exitCode: 1,
      exitKnown: true,
      outputText: output,
      cause: "failed",
    });
    expect(prompt).toContain("Command: git status");
    expect(prompt).toContain("Exit code: 1");
    expect(prompt).toContain("line 49");
    expect(prompt).not.toContain("line 0\n");
    expect(prompt.split("\n").filter((line) => line.startsWith("line "))).toHaveLength(40);
  });

  it("does not send an invented exit code", () => {
    const prompt = askFixPrompt({
      command: "echo hi",
      exitCode: 1,
      exitKnown: false,
      cause: "exit_unknown",
      outputText: "partial",
    });
    expect(prompt).toContain("Exit code: unknown");
    expect(prompt).not.toContain("Exit code: 1");
    expect(prompt).toContain("partial");
  });
});

describe("collapseRedraws and history labels", () => {
  it("keeps the last segment of a redrawn line", () => {
    expect(collapseRedraws("10%\r50%\r100%\ndone")).toBe("100%\ndone");
    expect(countOutputLines("10%\r50%\r100%\n")).toBe(1);
  });

  it("does not render the raw history status", () => {
    expect(historyStatusLabel("failure")).toBe("Did not work");
    expect(historyStatusLabel("success")).toBe("Finished");
    expect(historyStatusLabel("unknown")).toBe("Could not tell");
    expect(historyStatusLabel("failure").toLowerCase()).not.toBe("failure");
  });
});
