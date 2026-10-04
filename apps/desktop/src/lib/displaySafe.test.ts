import { describe, expect, it } from "vitest";
import { escapeForNote, escapeForTerminal } from "./displaySafe";

describe("escapeForNote", () => {
  it("keeps line breaks that the activity log can show", () => {
    const prompt = "This command did not work.\n\nCommand: Get-ChildItem";
    expect(escapeForNote(prompt)).toBe(prompt);
    expect(escapeForNote(prompt)).not.toContain("\\n");
    expect(escapeForTerminal(prompt)).toContain("\\n");
  });

  it("turns a Windows line ending into one break", () => {
    expect(escapeForNote("one\r\ntwo")).toBe("one\ntwo");
  });

  it("still writes out other controls", () => {
    expect(escapeForNote("a\u001bb")).toBe("a^[b");
  });
});
