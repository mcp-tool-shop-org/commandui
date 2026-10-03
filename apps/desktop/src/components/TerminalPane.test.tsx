import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";

type KeyHandler = (event: KeyboardEvent) => boolean;
const captured: { handler: KeyHandler | null } = { handler: null };

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown> = {};
    textarea = document.createElement("textarea");
    loadAddon() {}
    open() {}
    write() {}
    clear() {}
    reset() {}
    focus() {}
    dispose() {}
    onData() {
      return { dispose() {} };
    }
    attachCustomKeyEventHandler(handler: KeyHandler) {
      captured.handler = handler;
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));

import { TerminalPane } from "./TerminalPane";

function key(
  key: string,
  opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {},
  type = "keydown",
): KeyboardEvent {
  return new KeyboardEvent(type, { key, ...opts });
}

describe("TerminalPane custom key handler", () => {
  beforeEach(() => {
    captured.handler = null;
    globalThis.ResizeObserver ??= class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
    render(<TerminalPane />);
  });

  it("registers a handler", () => {
    expect(captured.handler).toBeTypeOf("function");
  });

  it.each(["K", "X", "W", "k"])("returns the Ctrl+Shift+%s chord to the app", (letter) => {
    expect(captured.handler!(key(letter, { ctrlKey: true, shiftKey: true }))).toBe(false);
  });

  it.each(["C", "V", "c", "v"])("keeps Ctrl+Shift+%s (copy/paste) in the terminal", (letter) => {
    expect(captured.handler!(key(letter, { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it("passes plain Ctrl+K to the shell", () => {
    expect(captured.handler!(key("k", { ctrlKey: true }))).toBe(true);
  });

  it("passes Ctrl+Alt+Shift+K to the terminal", () => {
    expect(captured.handler!(key("K", { ctrlKey: true, shiftKey: true, altKey: true }))).toBe(true);
  });

  it("passes non-keydown events through", () => {
    expect(captured.handler!(key("K", { ctrlKey: true, shiftKey: true }, "keyup"))).toBe(true);
    expect(captured.handler!(key("K", { ctrlKey: true, shiftKey: true }, "keypress"))).toBe(true);
  });

  it("passes Ctrl+Shift+non-letter to the terminal", () => {
    expect(captured.handler!(key("1", { ctrlKey: true, shiftKey: true }))).toBe(true);
  });
});
