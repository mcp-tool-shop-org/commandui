import { describe, expect, it } from "vitest";
import { memoryKindLabel, memoryScopeLabel, memoryValueLabel, suggestionLabel } from "./memoryLabels";

describe("memoryLabels", () => {
  it("names each kind in plain words and keeps an unknown kind as it is", () => {
    expect(memoryKindLabel("preferred_cwd")).toBe("Preferred workspace");
    expect(memoryKindLabel("recurring_command")).toBe("Frequent command");
    expect(memoryKindLabel("something_new")).toBe("something_new");
  });

  it("says where an item applies without printing the home folder", () => {
    expect(memoryScopeLabel("global")).toBe("Everywhere");
    expect(memoryScopeLabel("project")).toBe("Only in its project folder");
    expect(memoryScopeLabel("project", String.raw`C:\Users\Default\acme-api`)).toBe(String.raw`Only in ~\acme-api`);
  });

  it("shortens a folder value and a folder inside a suggestion, and nothing else", () => {
    const folder = String.raw`C:\Users\Default\acme-api`;
    expect(memoryValueLabel("preferred_cwd", folder)).toBe(String.raw`~\acme-api`);
    expect(memoryValueLabel("recurring_command", "git status")).toBe("git status");
    expect(suggestionLabel("preferred_cwd", `You've worked in ${folder} across 6 commands in 2 sessions`, folder)).toBe(
      String.raw`You've worked in ~\acme-api across 6 commands in 2 sessions`,
    );
    expect(suggestionLabel("workflow_pattern", "You often run: cd → ls", "cd → ls")).toBe("You often run: cd → ls");
  });
});
