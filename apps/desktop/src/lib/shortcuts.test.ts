import { describe, it, expect } from "vitest";
import {
  parseCombo,
  matchesEvent,
  resolveShortcut,
  hasModifier,
  isShellChord,
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

  it("cmd combo matches a metaKey event and a ctrlKey event", () => {
    const parsed = parseCombo("cmd+k");
    expect(matchesEvent(parsed, mockKeyEvent("k", { metaKey: true }))).toBe(true);
    expect(matchesEvent(parsed, mockKeyEvent("k", { ctrlKey: true }))).toBe(true);
    expect(matchesEvent(parsed, mockKeyEvent("k"))).toBe(false);
  });

  it("ctrl combo matches a metaKey event", () => {
    const parsed = parseCombo("ctrl+k");
    expect(matchesEvent(parsed, mockKeyEvent("k", { metaKey: true }))).toBe(true);
  });

  it("ctrl+k does not match ctrl+shift+k", () => {
    const parsed = parseCombo("ctrl+k");
    expect(
      matchesEvent(parsed, mockKeyEvent("k", { ctrlKey: true, shiftKey: true })),
    ).toBe(false);
  });

  it("shift+enter and bare enter do not match each other's events", () => {
    expect(matchesEvent(parseCombo("shift+enter"), mockKeyEvent("Enter"))).toBe(false);
    expect(
      matchesEvent(parseCombo("enter"), mockKeyEvent("Enter", { shiftKey: true })),
    ).toBe(false);
  });

  it("alt must match on both sides", () => {
    expect(matchesEvent(parseCombo("alt+k"), mockKeyEvent("k"))).toBe(false);
    expect(matchesEvent(parseCombo("alt+k"), mockKeyEvent("k", { altKey: true }))).toBe(true);
    expect(matchesEvent(parseCombo("k"), mockKeyEvent("k", { altKey: true }))).toBe(false);
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

  // Plan-only would stay null in a text zone even with the bare-key guard deleted.
  const bareKeyDefs = [makeDef("bare-a", "a", ["terminal", "composer", "plan"])];

  it("suppresses bare keys in terminal zone", () => {
    const match = resolveShortcut(bareKeyDefs, mockKeyEvent("a"), "terminal");
    expect(match).toBeNull();
  });

  it("suppresses bare keys in composer zone", () => {
    const match = resolveShortcut(bareKeyDefs, mockKeyEvent("a"), "composer");
    expect(match).toBeNull();
  });

  it("allows escape in text zones", () => {
    const defs = [makeDef("esc", "escape", ["terminal", "composer"])];
    const event = mockKeyEvent("Escape");
    expect(resolveShortcut(defs, event, "terminal")?.id).toBe("esc");
    expect(resolveShortcut(defs, event, "composer")?.id).toBe("esc");
  });

  it("allows a ctrl-modified combo in the terminal zone", () => {
    const defs = [makeDef("term-ctrl-k", "ctrl+k", ["terminal"])];
    const event = mockKeyEvent("k", { ctrlKey: true });
    expect(resolveShortcut(defs, event, "terminal")?.id).toBe("term-ctrl-k");
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
    const match = resolveShortcut(bareKeyDefs, mockKeyEvent("a"), "plan");
    expect(match?.id).toBe("bare-a");
  });

  function eventFrom(target: HTMLElement, key: string): KeyboardEvent {
    const event = new KeyboardEvent("keydown", { key, bubbles: true });
    Object.defineProperty(event, "target", { value: target });
    return event;
  }

  it("suppresses bare keys when the event target is a text field in plan zone", () => {
    const textarea = document.createElement("textarea");
    expect(resolveShortcut(bareKeyDefs, eventFrom(textarea, "a"), "plan")).toBeNull();
    const input = document.createElement("input");
    expect(resolveShortcut(bareKeyDefs, eventFrom(input, "a"), "plan")).toBeNull();
  });

  it("suppresses bare keys when document.activeElement is a text field in plan zone", () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    textarea.focus();
    try {
      expect(resolveShortcut(bareKeyDefs, mockKeyEvent("a"), "plan")).toBeNull();
    } finally {
      textarea.remove();
    }
  });

  it("allows bare keys in plan zone when the target is a non-text element", () => {
    const button = document.createElement("button");
    expect(resolveShortcut(bareKeyDefs, eventFrom(button, "a"), "plan")?.id).toBe("bare-a");
  });

  it("still resolves escape when the target is a textarea", () => {
    const defs = [makeDef("esc", "escape", ["plan"])];
    const textarea = document.createElement("textarea");
    expect(resolveShortcut(defs, eventFrom(textarea, "Escape"), "plan")?.id).toBe("esc");
  });

  describe("shell chords in the terminal zone", () => {
    const globalW = [makeDef("close-w", "ctrl+w", ["global"])];
    const ctrlW = () => mockKeyEvent("w", { ctrlKey: true });

    it("does not resolve a global plain ctrl+letter in the terminal zone", () => {
      expect(resolveShortcut(globalW, ctrlW(), "terminal")).toBeNull();
    });

    it("still resolves the same global def in composer and plan zones", () => {
      expect(resolveShortcut(globalW, ctrlW(), "composer")?.id).toBe("close-w");
      expect(resolveShortcut(globalW, ctrlW(), "plan")?.id).toBe("close-w");
    });

    it("resolves a plain ctrl+letter global with no zone", () => {
      expect(resolveShortcut(globalW, ctrlW(), null)?.id).toBe("close-w");
    });

    it("resolves a global ctrl+shift+letter in the terminal zone", () => {
      const defs = [makeDef("close-x", "ctrl+shift+x", ["global"])];
      const event = mockKeyEvent("X", { ctrlKey: true, shiftKey: true });
      expect(resolveShortcut(defs, event, "terminal")?.id).toBe("close-x");
    });

    // F-c3b3633e: plain Ctrl+digit and Ctrl+, are shell chords too. Ctrl+Enter
    // and Escape still resolve here; AppShell's `when` guards keep them from
    // approving or rejecting a plan while the terminal has focus.
    it("keeps plain ctrl+digit and ctrl+comma with the shell in the terminal zone", () => {
      const defs = [makeDef("d1", "ctrl+1", ["global"]), makeDef("set", "ctrl+,", ["global"])];
      expect(resolveShortcut(defs, mockKeyEvent("1", { ctrlKey: true }), "terminal")).toBeNull();
      expect(resolveShortcut(defs, mockKeyEvent(",", { ctrlKey: true }), "terminal")).toBeNull();
      expect(resolveShortcut(defs, mockKeyEvent("1", { ctrlKey: true }), "composer")?.id).toBe("d1");
    });

    it("still resolves global ctrl+enter and escape in the terminal zone", () => {
      const defs = [makeDef("ce", "ctrl+enter", ["global"]), makeDef("esc", "escape", ["global"])];
      expect(resolveShortcut(defs, mockKeyEvent("Enter", { ctrlKey: true }), "terminal")?.id).toBe("ce");
      expect(resolveShortcut(defs, mockKeyEvent("Escape"), "terminal")?.id).toBe("esc");
    });

    it("lets a terminal-zone-specific ctrl+letter def win over the skip", () => {
      const defs = [
        makeDef("global-k", "ctrl+k", ["global"]),
        makeDef("term-k", "ctrl+k", ["terminal"]),
      ];
      expect(resolveShortcut(defs, mockKeyEvent("k", { ctrlKey: true }), "terminal")?.id).toBe("term-k");
    });
  });

  describe("isShellChord", () => {
    it.each([
      ["k", { ctrlKey: true }, true],
      ["K", { ctrlKey: true }, true],
      ["k", { ctrlKey: true, shiftKey: true }, false],
      ["k", { ctrlKey: true, altKey: true }, false],
      ["k", { metaKey: true }, false],
      ["k", { ctrlKey: true, metaKey: true }, false],
      ["1", { ctrlKey: true }, true],
      [",", { ctrlKey: true }, true],
      ["1", { ctrlKey: true, shiftKey: true }, false],
      ["Enter", { ctrlKey: true }, false],
      ["k", {}, false],
    ])("key %s with %j is %s", (key, opts, expected) => {
      expect(isShellChord(mockKeyEvent(key, opts))).toBe(expected);
    });
  });

});
