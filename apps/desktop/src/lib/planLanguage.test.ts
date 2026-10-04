import { describe, expect, it } from "vitest";
import {
  SAFETY_FLAGS,
  confirmationMatches,
  confirmationPhrase,
  explainCommand,
  flagInWords,
  riskInWords,
} from "./planLanguage";

describe("plan language", () => {
  it("writes every safety flag in plain words", () => {
    for (const code of SAFETY_FLAGS) {
      const words = flagInWords(code);
      expect(words).not.toBe(code);
      expect(words.length).toBeGreaterThan(8);
      expect(words).not.toMatch(/DESTRUCTIVE|PRIVILEGE|NETWORK_ACCESS|HIGH_RISK/);
    }
    expect(flagInWords("DESTRUCTIVE_OPERATION")).toBe("Deletes files: cannot be undone");
    expect(flagInWords("PRIVILEGE_ESCALATION")).toBe("Runs with higher permissions");
    expect(flagInWords("HIGH_RISK_COMMAND")).toBe("Can make a change that is hard to undo");
    expect(flagInWords("NETWORK_ACCESS")).toBe("Uses the network");
    expect(flagInWords("SOMETHING_ELSE")).toBe("This needs a careful look before it runs");
  });

  it("says what a command does, piece by piece, and lists a delete", () => {
    const status = explainCommand("git status --short");
    expect(status.sentence).toBe("Shows a short list of what changed in this folder.");
    expect(status.parts.map((part) => part.piece)).toEqual(["git", "status", "--short"]);
    expect(status.touches).toEqual([]);

    const deletion = explainCommand('rm notes.txt "old log.txt"');
    expect(deletion.sentence).toBe("Deletes 2 files or folders.");
    expect(deletion.touches).toEqual(["notes.txt", "old log.txt"]);
    expect(riskInWords("high", true, false)).toBe("Deletes files: cannot be undone");
    expect(riskInWords("high", false, false)).toBe("High risk. This can be hard to undo.");
    expect(riskInWords("low", false, false)).toBe("Low risk. Easy to undo.");
  });

  it("uses the folder name as the high-risk confirmation", () => {
    expect(confirmationPhrase("C:\\Work\\notes")).toBe("notes");
    expect(confirmationPhrase("/work/notes/")).toBe("notes");
    expect(confirmationPhrase("C:\\")).toBe("confirm");
    expect(confirmationPhrase(undefined)).toBe("confirm");
    expect(confirmationMatches(" Notes ", "notes")).toBe(true);
    expect(confirmationMatches("nope", "notes")).toBe(false);
  });
});
