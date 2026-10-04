import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { TERMINAL_DARK, TERMINAL_LIGHT } from "../lib/terminalTheme";

const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "globals.css"), "utf8");

function channel(hex: string, index: number): number {
  const value = parseInt(hex.slice(1 + index * 2, 3 + index * 2), 16) / 255;
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  return 0.2126 * channel(hex, 0) + 0.7152 * channel(hex, 1) + 0.0722 * channel(hex, 2);
}

function ratio(a: string, b: string): number {
  const lighter = Math.max(luminance(a), luminance(b));
  const darker = Math.min(luminance(a), luminance(b));
  return (lighter + 0.05) / (darker + 0.05);
}

function variables(block: string): Record<string, string> {
  const found: Record<string, string> = {};
  for (const match of block.matchAll(/(--[a-z0-9-]+):\s*(#[0-9a-fA-F]{6})/g)) {
    found[match[1]] = match[2].toLowerCase();
  }
  return found;
}

function themeBlock(source: string, light: boolean): string {
  if (!light) {
    const start = source.indexOf(":root {");
    const end = source.indexOf("}", start);
    return source.slice(start, end);
  }
  const media = source.indexOf("@media (prefers-color-scheme: light)");
  const start = source.indexOf(":root {", media);
  const end = source.indexOf("}", start);
  return source.slice(start, end);
}

const dark = variables(themeBlock(css, false));
const light = variables(themeBlock(css, true));

const textPairs = [
  ["--text-primary", "--bg-primary"],
  ["--text-primary", "--bg-secondary"],
  ["--text-primary", "--bg-card"],
  ["--text-muted", "--bg-primary"],
  ["--text-muted", "--bg-secondary"],
  ["--text-muted", "--bg-card"],
  ["--accent", "--bg-primary"],
  ["--accent", "--bg-secondary"],
  ["--accent", "--bg-card"],
  ["--success", "--bg-primary"],
  ["--success", "--bg-card"],
  ["--success", "--bg-secondary"],
  ["--failure", "--bg-primary"],
  ["--failure", "--bg-card"],
  ["--failure", "--bg-secondary"],
  ["--info", "--bg-primary"],
  ["--info", "--bg-card"],
  ["--warning", "--bg-primary"],
  ["--warning", "--bg-card"],
  ["--warning", "--bg-secondary"],
  ["--term-fg", "--term-bg"],
] as const;

const controlPairs = [
  ["--border", "--bg-primary"],
  ["--border", "--bg-secondary"],
  ["--border", "--bg-card"],
  ["--accent", "--bg-primary"],
  ["--accent", "--bg-card"],
  ["--on-accent", "--accent"],
] as const;

describe("contrast tokens", () => {
  it("keeps text at 4.5:1 and controls at 3:1 in both themes", () => {
    for (const theme of [dark, light]) {
      for (const [fg, bg] of textPairs) {
        const score = ratio(theme[fg], theme[bg]);
        expect(score, `${fg} on ${bg} (${theme[fg]} on ${theme[bg]})`).toBeGreaterThanOrEqual(4.5);
      }
      for (const [fg, bg] of controlPairs) {
        const score = ratio(theme[fg], theme[bg]);
        expect(score, `${fg} on ${bg} (${theme[fg]} on ${theme[bg]})`).toBeGreaterThanOrEqual(3);
      }
    }
  });

  it("uses the same terminal colours the shell draws", () => {
    expect(dark["--term-bg"]).toBe(TERMINAL_DARK.background);
    expect(dark["--term-fg"]).toBe(TERMINAL_DARK.foreground);
    expect(light["--term-bg"]).toBe(TERMINAL_LIGHT.background);
    expect(light["--term-fg"]).toBe(TERMINAL_LIGHT.foreground);
  });

  it("scales with zoom instead of clipping, and honours contrast themes and reduced motion", () => {
    const shell = css.slice(css.indexOf(".app-shell {"), css.indexOf(".topbar {"));
    expect(shell).toMatch(/zoom:\s*var\(--ui-scale,\s*1\)/);
    expect(shell).toMatch(/width:\s*100%/);
    expect(shell).toMatch(/height:\s*100%/);
    expect(shell).not.toContain("calc(100% / var(--ui-scale");
    expect(css).not.toContain("calc(100% / var(--ui-scale");
    expect(css).toContain("@media (forced-colors: active)");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toMatch(/prefers-reduced-motion:\s*reduce\)[\s\S]*animation:\s*none/);
    expect(css).toMatch(/button\s*\{[^}]*min-width:\s*24px/);
    expect(css).toMatch(/button\s*\{[^}]*min-height:\s*24px/);
  });
});
