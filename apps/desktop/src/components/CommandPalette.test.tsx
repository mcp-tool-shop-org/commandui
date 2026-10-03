import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandPalette } from "./CommandPalette";

function setup(labels = ["Alpha", "Beta", "Gamma"]) {
  const fns = labels.map(() => vi.fn());
  const onClose = vi.fn();
  const actions = labels.map((label, i) => ({ id: label, label, action: fns[i] }));
  render(<CommandPalette isOpen onClose={onClose} actions={actions} />);
  const input = screen.getByPlaceholderText(/type a command/i);
  return { fns, onClose, input };
}

const selectedLabel = () =>
  screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")
    ?.textContent;

describe("CommandPalette keyboard handling", () => {
  it("does nothing on arrows or Enter when the filter matches nothing", async () => {
    const { fns, onClose, input } = setup();
    await userEvent.type(input, "zzz");
    expect(screen.getByText(/no matching commands/i)).toBeInTheDocument();

    expect(() => {
      fireEvent.keyDown(input, { key: "ArrowDown" });
      fireEvent.keyDown(input, { key: "ArrowUp" });
      fireEvent.keyDown(input, { key: "Enter" });
    }).not.toThrow();
    for (const fn of fns) expect(fn).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();

    // Selection did not become NaN: clearing the filter selects a real row.
    await userEvent.clear(input);
    expect(selectedLabel()).toBe("Alpha");
  });

  it("runs the first action once after the query is cleared", async () => {
    const { fns, onClose, input } = setup();
    await userEvent.type(input, "zzz");
    await userEvent.clear(input);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(fns[0]).toHaveBeenCalledTimes(1);
    expect(fns[1]).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("runs the surviving row when the filter narrows below the selection", async () => {
    const { fns, input } = setup();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(selectedLabel()).toBe("Gamma");

    await userEvent.type(input, "alp");
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(fns[0]).toHaveBeenCalledTimes(1);
    expect(fns[1]).not.toHaveBeenCalled();
    expect(fns[2]).not.toHaveBeenCalled();
  });

  it("wraps ArrowDown from the last row to the first and ArrowUp back", () => {
    const { input } = setup();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(selectedLabel()).toBe("Gamma");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(selectedLabel()).toBe("Alpha");
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(selectedLabel()).toBe("Gamma");
  });
});
