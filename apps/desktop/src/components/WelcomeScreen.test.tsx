import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { WelcomeScreen } from "./WelcomeScreen";

function setup(showAtStartup = true) {
  const onStart = vi.fn();
  const onShowAtStartupChange = vi.fn();
  render(
    <WelcomeScreen
      onStart={onStart}
      showAtStartup={showAtStartup}
      onShowAtStartupChange={onShowAtStartupChange}
      plannerModel="qwen2.5:14b"
    />,
  );
  return { onStart, onShowAtStartupChange };
}

describe("WelcomeScreen", () => {
  it("is a labelled dialog that explains the three things the app does", () => {
    setup();
    const dialog = screen.getByRole("dialog", { name: "Welcome to CommandUI" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByRole("heading", { name: "Run commands" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Ask in plain words" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Keep what works" })).toBeInTheDocument();
    expect(screen.getByText(/Nothing\s+runs until you approve it/)).toBeInTheDocument();
  });

  it("names the local model Ask needs and what happens without it", () => {
    setup();
    expect(screen.getByText("qwen2.5:14b")).toBeInTheDocument();
    expect(screen.getByText(/does not invent a command/)).toBeInTheDocument();
    expect(screen.queryByText(/built-in planner/)).not.toBeInTheDocument();
  });

  it("lists the shortcuts the app really binds", () => {
    setup();
    const keys = screen.getByRole("list", { name: "Keyboard shortcuts" });
    const shown = Array.from(keys.querySelectorAll("kbd")).map((k) => k.textContent);
    expect(shown).toEqual(["Ctrl+K", "Ctrl+J", "Ctrl+T", "Ctrl+Enter"]);
  });

  it("focuses Get started, and starts on click, Enter or Escape", () => {
    const { onStart } = setup();
    const start = screen.getByRole("button", { name: "Get started" });
    expect(start).toHaveFocus();
    fireEvent.click(start);
    expect(onStart).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(start, { key: "Escape" });
    expect(onStart).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(start, { key: "Enter" });
    expect(onStart).toHaveBeenCalledTimes(3);
  });

  it("does not start on Enter in the checkbox", () => {
    const { onStart } = setup();
    fireEvent.keyDown(screen.getByRole("checkbox"), { key: "Enter" });
    expect(onStart).not.toHaveBeenCalled();
  });

  it("is modal: keys aimed outside are held and focus comes back", () => {
    const outside = document.createElement("textarea");
    document.body.appendChild(outside);
    try {
      const { onStart } = setup();
      const start = screen.getByRole("button", { name: "Get started" });
      // The terminal takes focus once its shell is ready.
      outside.focus();
      expect(start).toHaveFocus();
      // A key that still lands outside never reaches it.
      const typed = vi.fn();
      outside.addEventListener("keydown", typed);
      const ev = new KeyboardEvent("keydown", { key: "g", bubbles: true, cancelable: true });
      outside.dispatchEvent(ev);
      expect(ev.defaultPrevented).toBe(true);
      expect(typed).not.toHaveBeenCalled();
      expect(start).toHaveFocus();
      // Escape from outside closes it.
      outside.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
      expect(onStart).toHaveBeenCalledTimes(1);
    } finally {
      outside.remove();
    }
  });

  it("stops holding keys once closed", () => {
    const outside = document.createElement("textarea");
    document.body.appendChild(outside);
    try {
      const { unmount } = render(
        <WelcomeScreen onStart={() => {}} showAtStartup onShowAtStartupChange={() => {}} plannerModel="m" />,
      );
      unmount();
      const ev = new KeyboardEvent("keydown", { key: "g", bubbles: true, cancelable: true });
      outside.dispatchEvent(ev);
      expect(ev.defaultPrevented).toBe(false);
    } finally {
      outside.remove();
    }
  });

  it("reports the show-at-startup choice both ways", () => {
    const first = setup(true);
    const box = screen.getByRole("checkbox", { name: "Show this when CommandUI opens" });
    expect(box).toBeChecked();
    fireEvent.click(box);
    expect(first.onShowAtStartupChange).toHaveBeenLastCalledWith(false);
  });

  it("shows the box unticked when the user turned it off", () => {
    const { onShowAtStartupChange } = setup(false);
    const box = screen.getByRole("checkbox", { name: "Show this when CommandUI opens" });
    expect(box).not.toBeChecked();
    fireEvent.click(box);
    expect(onShowAtStartupChange).toHaveBeenLastCalledWith(true);
  });
});
