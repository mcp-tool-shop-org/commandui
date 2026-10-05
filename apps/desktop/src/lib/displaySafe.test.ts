import { describe, expect, it } from "vitest";
import {
  commandProblem,
  escapeForNote,
  describeHiddenChars,
  escapeForTerminal,
  findHiddenChars,
  formatCodePoint,
  hasHiddenChars,
  markHiddenChars,
  stripHiddenChars,
} from "./displaySafe";

describe("displaySafe", () => {
  it("names each class of character that would not draw as itself", () => {
    const samples: Array<[string, string]> = [
      ["\u202e", "bidirectional control"],
      ["\u2066", "bidirectional control"],
      ["\u200e", "bidirectional control"],
      ["\u061c", "bidirectional control"],
      ["\u200b", "zero-width or invisible"],
      ["\u2060", "zero-width or invisible"],
      ["\u206a", "zero-width or invisible"],
      ["\ufeff", "zero-width or invisible"],
      ["\u00ad", "zero-width or invisible"],
      ["\u180e", "zero-width or invisible"],
      ["\u2028", "line or paragraph separator"],
      ["\u{e0001}", "zero-width or invisible"],
      ["\ufe00", "zero-width or invisible"],
      ["\u{e0100}", "zero-width or invisible"],
      ["\u034f", "zero-width or invisible"],
      ["\u115f", "zero-width or invisible"],
      ["\u1160", "zero-width or invisible"],
      ["\u3164", "zero-width or invisible"],
      ["\uffa0", "zero-width or invisible"],
      ["\u2800", "zero-width or invisible"],
      ["\ufff9", "zero-width or invisible"],
      ["\u0600", "zero-width or invisible"],
    ];
    for (const [char, kind] of samples) {
      expect(findHiddenChars(`a${char}b`)).toEqual([
        { char, codePoint: char.codePointAt(0), kind },
      ]);
    }
    expect(findHiddenChars("plain")).toEqual([]);
    expect(hasHiddenChars("plain")).toBe(false);
    expect(describeHiddenChars("plain")).toBeNull();
  });

  it("describes a mix once per class and once per code point", () => {
    const text = `go\u202e\u202e\u200b`;
    expect(describeHiddenChars(text)).toBe(
      "3 hidden characters (bidirectional control, zero-width or invisible): <U+202E> <U+200B>",
    );
    expect(formatCodePoint(0xa)).toBe("<U+000A>");
  });

  it("strips hidden characters and marks them without touching the rest", () => {
    expect(stripHiddenChars("a\u200bb")).toBe("ab");
    expect(markHiddenChars("a\u200bb")).toBe("a<U+200B>b");
  });

  it("shows controls the terminal would obey as visible markers", () => {
    expect(escapeForTerminal("a\nb\rc\td\x1b\x7f\x80\u200be")).toBe(
      "a\\nb\\rc\\td^[^?\\x80<U+200B>e",
    );
  });

  it("refuses a command the shell would not run as one visible line", () => {
    expect(commandProblem("git status")).toBeNull();
    expect(commandProblem("git\nstatus")).toMatch(/more than one line/);
    expect(commandProblem("git\rstatus")).toMatch(/more than one line/);
    expect(commandProblem("git\tstatus")).toMatch(/control character/);
    expect(commandProblem("git\u202estatus")).toMatch(/hidden character/);
  });
});

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
