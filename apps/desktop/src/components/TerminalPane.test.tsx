import { describe, it, expect, vi, beforeEach } from "vitest";
import { createRef } from "react";
import { render, screen } from "@testing-library/react";

type KeyHandler = (event: KeyboardEvent) => boolean;
type DataHandler = (data: string) => void;
type BufferHandler = (buffer: { type: string }) => void;
const captured: {
  handler: KeyHandler | null;
  onData: DataHandler | null;
  onBuffer: BufferHandler | null;
  writes: string[];
  cleared: number;
  resetCount: number;
  focused: number;
  options: Record<string, unknown> | null;
} = {
  handler: null,
  onData: null,
  onBuffer: null,
  writes: [],
  cleared: 0,
  resetCount: 0,
  focused: 0,
  options: null,
};

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown> = { convertEol: true };
    textarea = document.createElement("textarea");
    constructor(options?: Record<string, unknown>) {
      this.options = { convertEol: true, ...(options ?? {}) };
      captured.options = this.options;
    }
    buffer = {
      onBufferChange(handler: BufferHandler) {
        captured.onBuffer = handler;
        return { dispose() {} };
      },
    };
    loadAddon() {}
    open() {}
    write(data: string, callback?: () => void) {
      captured.writes.push(data);
      if (data.includes("\x1b[6n")) captured.onData?.("\x1b[1;1R");
      callback?.();
    }
    clear() {
      captured.cleared += 1;
    }
    reset() {
      captured.resetCount += 1;
    }
    focus() {
      captured.focused += 1;
    }
    dispose() {}
    onData(handler: DataHandler) {
      captured.onData = handler;
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

import { isTerminalAutoReply, TerminalPane, type TerminalPaneHandle } from "./TerminalPane";

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
    captured.onData = null;
    captured.onBuffer = null;
    captured.writes = [];
    captured.cleared = 0;
    captured.resetCount = 0;
    captured.focused = 0;
    captured.options = null;
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

  it("turns on screen reader mode so typing, dictation, and paste stay in the terminal", () => {
    expect(captured.options?.screenReaderMode).toBe(true);
    // The shell zoom is the text scale. A 28px cell under that zoom would draw the glyphs twice.
    expect(captured.options?.fontSize).toBe(14);
  });

  it("follows the light theme", () => {
    const previous = window.matchMedia;
    window.matchMedia = ((query: string) => ({
      matches: query.includes("light"),
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
      onchange: null,
    })) as typeof window.matchMedia;
    render(<TerminalPane />);
    const theme = captured.options?.theme as { background?: string; foreground?: string };
    expect(theme.background).toBe("#ffffff");
    expect(theme.foreground).toBe("#111827");
    window.matchMedia = previous;
  });

  it("stops the cursor blink when reduced motion is on, even while a command runs", () => {
    const previous = window.matchMedia;
    window.matchMedia = ((query: string) => ({
      matches: query.includes("reduce"),
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
      onchange: null,
    })) as typeof window.matchMedia;
    render(<TerminalPane executionStatus="running" />);
    expect(captured.options?.cursorBlink).toBe(false);
    window.matchMedia = previous;
  });

  it("does not render the status word", () => {
    const { unmount } = render(<TerminalPane executionStatus="failure" />);
    expect(document.body.textContent?.toLowerCase() ?? "").not.toContain("failure");
    unmount();
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

describe("isTerminalAutoReply", () => {
  it("recognises xterm's own reports and nothing a person types", () => {
    expect(isTerminalAutoReply("\x1b[1;1R")).toBe(true);
    expect(isTerminalAutoReply("\x1b[?1;2c")).toBe(true);
    expect(isTerminalAutoReply("\x1b[0n")).toBe(true);
    expect(isTerminalAutoReply("\x1b]0;title\x07")).toBe(true);
    expect(isTerminalAutoReply("ls\r")).toBe(false);
    expect(isTerminalAutoReply("\x1b[A")).toBe(false);
  });
});

describe("TerminalPane replay and status", () => {
  beforeEach(() => {
    captured.writes = [];
    captured.cleared = 0;
    captured.resetCount = 0;
    captured.focused = 0;
    captured.onData = null;
    captured.onBuffer = null;
    globalThis.ResizeObserver ??= class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
  });

  it("drops an auto-reply during replay, then delivers a keystroke and a queued live write", () => {
    const onData = vi.fn();
    const onResize = vi.fn();
    const ref = createRef<TerminalPaneHandle>();
    const view = render(
      <TerminalPane ref={ref} sessionId="s1" onData={onData} onResize={onResize} autoFocus />,
    );

    expect(captured.focused).toBeGreaterThan(0);
    expect(onResize).toHaveBeenCalledWith(80, 24);

    const original = captured.onData;
    captured.onData = (data) => {
      if (data === "\x1b[1;1R") ref.current?.write("held");
      original?.(data);
    };
    ref.current?.replay(["\x1b[6n", "prompt"]);
    expect(onData).not.toHaveBeenCalled();
    expect(captured.writes).toContain("held");
    expect(captured.cleared).toBeGreaterThan(0);

    captured.onData?.("ls\r");
    expect(onData).toHaveBeenCalledWith("ls\r");

    captured.onBuffer?.({ type: "alternate" });
    view.rerender(
      <TerminalPane
        ref={ref}
        sessionId="s1"
        executionStatus="running"
        onData={onData}
      />,
    );

    view.rerender(<TerminalPane ref={ref} sessionId="s2" onData={onData} />);
    expect(captured.resetCount).toBeGreaterThan(1);
  });
});
