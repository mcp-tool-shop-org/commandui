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

  it("explains PowerShell command, path, and access failures from real output", () => {
    // Captured 2026-10-04 from Windows PowerShell 5.1 and PowerShell 7 on this machine.
    const windowsPowerShellNotFound = [
      "notacommand987xyz : The term 'notacommand987xyz' is not recognized as the name of a cmdlet, function, script file, or ",
      "operable program. Check the spelling of the name, or if a path was included, verify that the path is correct and try ",
      "again.",
      "At line:1 char:1",
      "+ notacommand987xyz",
      "+ ~~~~~~~~~~~~~~~~~",
      "    + CategoryInfo          : ObjectNotFound: (notacommand987xyz:String) [], CommandNotFoundException",
      "    + FullyQualifiedErrorId : CommandNotFoundException",
    ].join("\n");
    const powerShell7NotFound = [
      "notacommand987xyz: The term 'notacommand987xyz' is not recognized as a name of a cmdlet, function, script file, or executable program.",
      "Check the spelling of the name, or if a path was included, verify that the path is correct and try again.",
    ].join("\n");
    const windowsPowerShellMissing = [
      "Get-Item : Cannot find path 'C:\\no\\such\\commandui-walkthrough' because it does not exist.",
      "At line:1 char:1",
      "+ Get-Item -LiteralPath 'C:\\no\\such\\commandui-walkthrough'",
      "+ ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
      "    + CategoryInfo          : ObjectNotFound: (C:\\no\\such\\commandui-walkthrough:String) [Get-Item], ItemNotFoundException",
      "    + FullyQualifiedErrorId : PathNotFound,Microsoft.PowerShell.Commands.GetItemCommand",
    ].join("\n");
    const powerShell7Missing =
      "Set-Location: Cannot find path 'C:\\no\\such\\commandui-walkthrough' because it does not exist.";
    const windowsPowerShellDenied = [
      "Get-ChildItem : Access to the path 'C:\\System Volume Information' is denied.",
      "At line:1 char:1",
      "+ Get-ChildItem -LiteralPath 'C:\\System Volume Information'",
      "+ ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
      "    + CategoryInfo          : PermissionDenied: (C:\\System Volume Information:String) [Get-ChildItem], UnauthorizedAccessException",
      "    + FullyQualifiedErrorId : DirUnauthorizedAccessError,Microsoft.PowerShell.Commands.GetChildItemCommand",
    ].join("\n");
    const powerShell7Denied = "Get-ChildItem: Access to the path 'C:\\System Volume Information' is denied.";

    expect(
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: windowsPowerShellNotFound }).reason,
    ).toBe("The shell could not find that command.");
    expect(
      describeResult({
        phase: "failure",
        exitCode: 1,
        exitKnown: true,
        outputText: `\u001b[31m${powerShell7NotFound}\u001b[m`,
      }).reason,
    ).toBe("The shell could not find that command.");
    expect(
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: windowsPowerShellMissing }).reason,
    ).toBe("A file or folder in that command is not there.");
    expect(
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: powerShell7Missing }).reason,
    ).toBe("A file or folder in that command is not there.");
    expect(
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: windowsPowerShellDenied }).reason,
    ).toBe("CommandUI was not allowed to do that.");
    expect(
      describeResult({ phase: "failure", exitCode: 1, exitKnown: true, outputText: powerShell7Denied }).reason,
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

  it("drops cursor and colour codes and keeps a listing", () => {
    const chrome = "\u001b[?25l\u001b[32m\u001b[1m\u001b[m>";
    expect(collapseRedraws(chrome)).toBe("");
    expect(countOutputLines(chrome)).toBe(0);
    expect(collapseRedraws(chrome)).not.toContain("[?25l");
    expect(collapseRedraws(chrome)).not.toContain("[32m");
    expect(collapseRedraws(chrome)).not.toContain("[1m");

    const listing =
      "\u001b[?25l\u001b[32m\u001b[1m\u001b[m>\rDirectory: C:\\Work\\logs\n" +
      "Mode  Name\r\n" +
      "----  ----\r\n" +
      "-a--- a.log\r\n" +
      "-a--- b.log\r\n";
    const shown = collapseRedraws(listing);
    expect(shown).toBe(["Directory: C:\\Work\\logs", "Mode  Name", "----  ----", "-a--- a.log", "-a--- b.log"].join("\n"));
    expect(shown).not.toContain("[?25l");
    expect(shown).not.toContain("[32m");
    expect(countOutputLines(listing)).toBe(5);
  });

  it("starts a block after this command's own echo", () => {
    const raw = [
      "> cd C:\\Work\\demo",
      "",
      "> Get-ChildItem logs",
      "",
      "Directory: C:\\Work\\demo\\logs",
      "Mode  Name",
      "-a--- a.log",
    ].join("\n");
    const shown = [
      "Directory: C:\\Work\\demo\\logs",
      "Mode  Name",
      "-a--- a.log",
    ].join("\n");
    expect(collapseRedraws(raw, "Get-ChildItem logs")).toBe(shown);
    expect(collapseRedraws(shown)).toBe(shown);
    expect(collapseRedraws(shown, "Get-ChildItem logs")).toBe(shown);
  });

  it("drops a leftover earlier echo when this command's echo is already gone", () => {
    const raw = ["> cd C:\\Work\\demo", "", "Directory: C:\\Work\\demo\\logs", "a.log"].join("\n");
    const shown = ["Directory: C:\\Work\\demo\\logs", "a.log"].join("\n");
    expect(collapseRedraws(raw, "Get-ChildItem logs")).toBe(shown);
    expect(collapseRedraws(shown)).toBe(shown);
  });

  it("keeps a line that starts with > once real output has begun", () => {
    const raw = ["> type readme", "", "> quoted line", "next"].join("\n");
    const shown = "> quoted line\nnext";
    expect(collapseRedraws(raw, "type readme")).toBe(shown);
    expect(collapseRedraws(shown)).toBe(shown);
  });

  it("does not count a prompt redraw as output", () => {
    const redraw = "\u001b[?25l\u001b[32m\u001b[1m\u001b[m> cd\r\n";
    expect(countOutputLines(redraw, "cd")).toBe(0);
    const result = describeResult({ phase: "success", outputText: redraw, command: "cd" });
    expect(result.headline).toBe("Finished. 0 lines of output.");
    expect(result.actions).toEqual([]);
    const boot =
      "\u001b[?9001h\u001b[?1004h\u001b[?25l\u001b[2J\u001b[m\u001b[H\u001b]0;Windows PowerShell\u0007\u001b[?25h";
    expect(countOutputLines(boot)).toBe(0);
    expect(collapseRedraws(boot)).not.toContain("[?25l");
    expect(collapseRedraws(boot)).not.toContain("[2J");
  });

  it("does not render the raw history status", () => {
    expect(historyStatusLabel("failure")).toBe("Did not work");
    expect(historyStatusLabel("success")).toBe("Finished");
    expect(historyStatusLabel("unknown")).toBe("Could not tell");
    expect(historyStatusLabel("failure").toLowerCase()).not.toBe("failure");
  });
});
