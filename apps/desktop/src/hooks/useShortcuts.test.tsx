import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useFocusStore } from "@commandui/state";
import type { ShortcutDef } from "../lib/shortcuts";
import { useShortcuts } from "./useShortcuts";

function Harness({ shortcuts }: { shortcuts: ShortcutDef[] }) {
  useShortcuts(shortcuts);
  return <button type="button">idle</button>;
}

describe("useShortcuts", () => {
  it("runs the matching action and keeps the key from doing anything else", () => {
    useFocusStore.setState({ currentZone: "composer", previousZone: null });
    const action = vi.fn();
    const shortcuts: ShortcutDef[] = [
      { id: "history", combo: "ctrl+h", context: ["global"], action },
    ];
    render(<Harness shortcuts={shortcuts} />);

    const event = new KeyboardEvent("keydown", { key: "h", ctrlKey: true, bubbles: true, cancelable: true });
    const prevented = !window.dispatchEvent(event);
    expect(action).toHaveBeenCalledOnce();
    expect(prevented).toBe(true);
    expect(screen.getByRole("button", { name: "idle" })).toBeInTheDocument();
  });

  it("leaves an unmatched key alone", () => {
    useFocusStore.setState({ currentZone: null, previousZone: null });
    const action = vi.fn();
    render(
      <Harness
        shortcuts={[{ id: "history", combo: "ctrl+h", context: ["global"], action }]}
      />,
    );
    const event = new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true, cancelable: true });
    window.dispatchEvent(event);
    expect(action).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });
});
