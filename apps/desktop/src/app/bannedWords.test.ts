import { describe, expect, it } from "vitest";

const files = import.meta.glob(
  ["../components/**/*.tsx", "../app/AppShell.tsx", "../lib/commandResult.ts", "../lib/commandError.ts", "../lib/planLanguage.ts"],
  { query: "?raw", import: "default", eager: true },
) as Record<string, string>;

const BANNED: Array<{ name: string; pattern: RegExp }> = [
  { name: "semantic", pattern: /\bsemantic\b/i },
  { name: "desync", pattern: /\bdesync(?:ed)?\b/i },
  { name: "booting", pattern: /\bbooting\b/i },
  { name: "plumbing", pattern: /\bplumbing\b/i },
  { name: "marker", pattern: /\bmarker\b/i },
  { name: "PTY", pattern: /\bPTY\b/ },
  { name: "exec", pattern: /\bexec\b/i },
  { name: "mock", pattern: /\bmock\b/i },
  { name: "Tauri", pattern: /\bTauri\b/ },
  { name: "backend", pattern: /\bbackend\b/i },
  { name: "database", pattern: /\bdatabase\b/i },
  { name: "[workflow:", pattern: /\[workflow:/i },
  { name: "[plan]", pattern: /\[plan\]/i },
  { name: "Command failed", pattern: /Command '[^']*' failed/ },
  { name: "DESTRUCTIVE_OPERATION", pattern: /\bDESTRUCTIVE_OPERATION\b/ },
  { name: "PRIVILEGE_ESCALATION", pattern: /\bPRIVILEGE_ESCALATION\b/ },
  { name: "HIGH_RISK_COMMAND", pattern: /\bHIGH_RISK_COMMAND\b/ },
  { name: "NETWORK_ACCESS", pattern: /\bNETWORK_ACCESS\b/ },
];

/** Whole-string tokens the program compares. A sentence that contains one still fails. */
const EXACT = new Set([
  "booting",
  "desynced",
  "mock",
  "semantic",
  "raw",
  "promoted",
  "ollama",
  "pty",
  "exec",
  "DESTRUCTIVE_OPERATION",
  "PRIVILEGE_ESCALATION",
  "HIGH_RISK_COMMAND",
  "NETWORK_ACCESS",
]);

function stringLiterals(source: string): string[] {
  const found: string[] = [];
  const s = source;
  let i = 0;

  function walk(stop: string | null): void {
    let depth = 0;
    while (i < s.length) {
      const c = s[i];
      if (stop === "}" && c === "{") depth += 1;
      if (stop === "}" && c === "}" && depth === 0) return;
      if (stop === "}" && c === "}") {
        depth -= 1;
        i += 1;
        continue;
      }
      if (c === "/" && s[i + 1] === "/") {
        while (i < s.length && s[i] !== "\n") i += 1;
        continue;
      }
      if (c === "/" && s[i + 1] === "*") {
        i += 2;
        while (i < s.length && !(s[i] === "*" && s[i + 1] === "/")) i += 1;
        i += 2;
        continue;
      }
      if (c === "'" || c === '"') {
        const quote = c;
        i += 1;
        let buf = "";
        while (i < s.length && s[i] !== quote) {
          if (s[i] === "\\") {
            buf += s[i + 1] ?? "";
            i += 2;
            continue;
          }
          buf += s[i];
          i += 1;
        }
        i += 1;
        found.push(buf);
        continue;
      }
      if (c === "`") {
        i += 1;
        let buf = "";
        while (i < s.length && s[i] !== "`") {
          if (s[i] === "\\") {
            buf += s[i + 1] ?? "";
            i += 2;
            continue;
          }
          if (s[i] === "$" && s[i + 1] === "{") {
            if (buf) found.push(buf);
            buf = "";
            i += 2;
            walk("}");
            if (s[i] === "}") i += 1;
            continue;
          }
          buf += s[i];
          i += 1;
        }
        if (buf) found.push(buf);
        i += 1;
        continue;
      }
      i += 1;
    }
  }

  walk(null);
  return found;
}

describe("banned words", () => {
  it("keeps internal words out of the text a person can see", () => {
    const hits: string[] = [];
    for (const [path, source] of Object.entries(files)) {
      if (path.includes(".test.")) continue;
      for (const text of stringLiterals(source)) {
        const trimmed = text.trim();
        if (EXACT.has(trimmed)) continue;
        for (const ban of BANNED) {
          if (ban.pattern.test(trimmed)) {
            hits.push(`${path}: ${ban.name} in ${JSON.stringify(trimmed.slice(0, 140))}`);
          }
        }
      }
    }
    expect(Object.keys(files).some((path) => path.endsWith("AppShell.tsx"))).toBe(true);
    expect(hits).toEqual([]);
  });
});
