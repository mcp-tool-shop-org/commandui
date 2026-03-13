import { describe, it, expect } from "vitest";
import {
  parseCombo,
  matchesEvent,
  resolveShortcut,
  hasModifier,
} from "./shortcuts";
import type { ShortcutDef } from "./shortcuts";

function mockKeyEvent(
  key: string,
  opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean } = {},
): KeyboardEvent {
  return new KeyboardEvent("keydown", { key, ...opts });
}

describe("parseCombo", () => {
  it("parses ctrl+k", () => {
    const p = parseCombo("ctrl+k");
    expect(p).toEqual({ ctrl: true, shift: false, alt: false, meta: false, key: "k" });
  });

  it("parses shift+enter", () => {
    const p = parseCombo("shift+enter");
    expect(p).toEqual({ ctrl: false, shift: true, alt: false, meta: false, key: "enter" });
  });

  it("parses bare escape", () => {
    const p = parseCombo("escape");
    expect(p).toEqual({ ctrl: false, shift: false, alt: false, meta: false, key: "escape" });
  });

  it("parses ctrl+shift+p", () => {
    const p = parseCombo("ctrl+shift+p");
    expect(p).toEqual({ ctrl: true, shift: true, alt: false, meta: false, key: "p" });
  });

  it("treats cmd as ctrl", () => {
    const p = parseCombo("cmd+k");
    expect(p.ctrl).toBe(true);
  });

  it("parses single letter", () => {
    const p = parseCombo("a");
    expect(p).toEqual({ ctrl: false, shift: false, alt: false, meta: false, key: "a" });
  });
});

describe("hasModifier", () => {
  it("returns true for ctrl combo", () => {
    expect(hasModifier(parseCombo("ctrl+k"))).toBe(true);
  });

  it("returns false for bare key", () => {
    expect(hasModifier(parseCombo("a"))).toBe(false);
  });
});

describe("matchesEvent", () => {
  it("matches ctrl+k", () => {
    const parsed = parseCombo("ctrl+k");
    const event = mockKeyEvent("k", { ctrlKey: true });
    expect(matchesEvent(parsed, event)).toBe(true);
  });

  it("does not match without modifier", () => {
    const parsed = parseCombo("ctrl+k");
    const event = mockKeyEvent("k");
    expect(matchesEvent(parsed, event)).toBe(false);
  });

  it("matches escape", () => {
    const parsed = parseCombo("escape");
    const event = mockKeyEvent("Escape");
    expect(matchesEvent(parsed, event)).toBe(true);
  });

  it("matches enter", () => {
    const parsed = parseCombo("enter");
    const event = mockKeyEvent("Enter");
    expect(matchesEvent(parsed, event)).toBe(true);
  });

  it("matches ctrl+shift+w", () => {
    const parsed = parseCombo("ctrl+shift+w");
    const event = mockKeyEvent("W", { ctrlKey: true, shiftKey: true });
    expect(matchesEvent(parsed, event)).toBe(true);
  });
});

describe("resolveShortcut", () => {
  const makeDef = (id: string, combo: string, context: string[], when?: () => boolean): ShortcutDef => ({
    id,
    combo,
    context: context as ShortcutDef["context"],
    when,
    action: () => {},
  });

  it("resolves a global shortcut", () => {
    const defs = [makeDef("palette", "ctrl+k", ["global"])];
    const event = mockKeyEvent("k", { ctrlKey: true });
    const match = resolveShortcut(defs, event, "composer");
    expect(match?.id).toBe("palette");
  });

  it("zone-specific beats global", () => {
    const defs = [
      makeDef("global-esc", "escape", ["global"]),
      makeDef("plan-esc", "escape", ["plan"]),
    ];
    const event = mockKeyEvent("Escape");
    const match = resolveShortcut(defs, event, "plan");
    expect(match?.id).toBe("plan-esc");
  });

  it("suppresses bare keys in terminal zone", () => {
    const defs = [makeDef("plan-a", "a", ["plan"])];
    const event = mockKeyEvent("a");
    const match = resolveShortcut(defs, event, "terminal");
    expect(match).toBeNull();
  });

  it("suppresses bare keys in composer zone", () => {
    const defs = [makeDef("plan-a", "a", ["plan"])];
    const event = mockKeyEvent("a");
    const match = resolveShortcut(defs, event, "composer");
    expect(match).toBeNull();
  });

  it("allows escape in text zones", () => {
    const defs = [makeDef("esc", "escape", ["global"])];
    const event = mockKeyEvent("Escape");
    const match = resolveShortcut(defs, event, "composer");
    expect(match?.id).toBe("esc");
  });

  it("respects when guard", () => {
    const defs = [makeDef("guarded", "ctrl+k", ["global"], () => false)];
    const event = mockKeyEvent("k", { ctrlKey: true });
    const match = resolveShortcut(defs, event, null);
    expect(match).toBeNull();
  });

  it("fires when guard returns true", () => {
    const defs = [makeDef("guarded", "ctrl+k", ["global"], () => true)];
    const event = mockKeyEvent("k", { ctrlKey: true });
    const match = resolveShortcut(defs, event, null);
    expect(match?.id).toBe("guarded");
  });

  it("returns null for unmatched event", () => {
    const defs = [makeDef("palette", "ctrl+k", ["global"])];
    const event = mockKeyEvent("j", { ctrlKey: true });
    expect(resolveShortcut(defs, event, null)).toBeNull();
  });

  it("allows bare keys in plan zone", () => {
    const defs = [makeDef("plan-a", "a", ["plan"])];
    const event = mockKeyEvent("a");
    const match = resolveShortcut(defs, event, "plan");
    expect(match?.id).toBe("plan-a");
  });
});
