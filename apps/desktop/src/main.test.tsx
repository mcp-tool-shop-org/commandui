import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, screen } from "@testing-library/react";
import { resetMockBridge } from "./lib/mockBridge";

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown> = {};
    textarea = document.createElement("textarea");
    loadAddon() {}
    open() {}
    write(_data: string, callback?: () => void) {
      callback?.();
    }
    clear() {}
    reset() {}
    focus() {}
    dispose() {}
    onData() {
      return { dispose() {} };
    }
    attachCustomKeyEventHandler() {}
    replay() {}
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));
vi.mock("./styles/globals.css", () => ({}));

describe("main", () => {
  afterEach(() => {
    cleanup();
    resetMockBridge();
  });

  it("mounts the shell and records an unhandled rejection", async () => {
    document.body.innerHTML = '<div id="root"></div>';
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
    const error = vi.spyOn(console, "error").mockImplementation(() => {});

    await import("./main");
    expect(await screen.findByText(/Welcome to CommandUI — Session 1/)).toBeInTheDocument();

    const reason = new Error("background blew up");
    const rejection = new Event("unhandledrejection");
    Object.defineProperty(rejection, "reason", { value: reason });
    window.dispatchEvent(rejection);
    expect(error).toHaveBeenCalledWith("[CommandUI] Unhandled promise rejection:", reason);
  });
});
