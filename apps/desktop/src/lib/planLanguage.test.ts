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

  it("explains a PowerShell pipeline part by part", () => {
    const explained = explainCommand(
      "Get-ChildItem -File -Filter *.log | Sort-Object Length -Descending | Select-Object -First 3",
    );
    expect(explained.sentence).toBe(
      "Lists files whose names match *.log, then sorts them by Length, largest first, then keeps the first 3.",
    );
    expect(explained.parts).toEqual([
      { piece: "Get-ChildItem", meaning: "Lists files and folders" },
      { piece: "-File", meaning: "Only files" },
      { piece: "-Filter", meaning: "Keeps names that match" },
      { piece: "*.log", meaning: "The name pattern" },
      { piece: "|", meaning: "Sends the results to the next step" },
      { piece: "Sort-Object", meaning: "Sorts the results" },
      { piece: "Length", meaning: "The value to sort by" },
      { piece: "-Descending", meaning: "Largest first" },
      { piece: "|", meaning: "Sends the results to the next step" },
      { piece: "Select-Object", meaning: "Chooses which results to keep" },
      { piece: "-First", meaning: "How many to keep from the start" },
      { piece: "3", meaning: "The number to keep" },
    ]);
    const filler = ["Passed to the program", "Changes how the command runs", "Runs Get-ChildItem"];
    for (const part of explained.parts) {
      expect(filler).not.toContain(part.meaning);
    }
    expect(explained.sentence).not.toContain("Runs Get-ChildItem");
  });

  it("does not invent a meaning for a command it does not know", () => {
    const explained = explainCommand("flarble --zzz quux");
    expect(explained.sentence).toBe("CommandUI cannot explain this command. Read it before you run it.");
    expect(explained.parts).toEqual([]);
    expect(explained.touches).toEqual([]);
  });

  it("explains the parts it knows and says the rest is not explained", () => {
    const explained = explainCommand("Get-ChildItem -File -NotARealSwitch");
    expect(explained.sentence).toBe(
      "Lists files. CommandUI cannot explain the rest of this command.",
    );
    expect(explained.parts.map((part) => part.piece)).toEqual(["Get-ChildItem", "-File"]);
    expect(explained.parts.map((part) => part.meaning)).not.toContain("Passed to the program");
    expect(explained.parts.map((part) => part.meaning)).not.toContain("Changes how the command runs");
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
