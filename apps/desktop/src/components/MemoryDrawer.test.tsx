import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MemoryItem } from "@commandui/domain";
import { MemoryDrawer } from "./MemoryDrawer";

function item(partial: Partial<MemoryItem> & Pick<MemoryItem, "id">): MemoryItem {
  return {
    scope: "project",
    projectRoot: String.raw`C:\Users\Default\app`,
    kind: "preferred_cwd",
    key: "workspace",
    value: "/work/app",
    confidence: 0.9,
    source: "accepted",
    createdAt: "2026-10-04T00:00:00Z",
    updatedAt: "2026-10-04T00:00:00Z",
    ...partial,
  };
}

describe("MemoryDrawer", () => {
  it("renders nothing while it is closed", () => {
    render(
      <MemoryDrawer isOpen={false} items={[]} onClose={vi.fn()} onDelete={vi.fn()} />,
    );
    expect(screen.queryByText("Memory")).toBeNull();
  });

  it("shows skeletons while memory is loading", () => {
    render(
      <MemoryDrawer isOpen items={[]} onClose={vi.fn()} onDelete={vi.fn()} loading />,
    );
    expect(document.querySelectorAll(".skeleton-item")).toHaveLength(3);
    expect(screen.queryByText("No saved memory yet.")).toBeNull();
  });

  it("says when nothing is saved", () => {
    render(<MemoryDrawer isOpen items={[]} onClose={vi.fn()} onDelete={vi.fn()} />);
    expect(screen.getByText("No saved memory yet.")).toBeInTheDocument();
  });

  it("shows each item and deletes only the one the user picks", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    const onClose = vi.fn();
    render(
      <MemoryDrawer
        isOpen
        onClose={onClose}
        onDelete={onDelete}
        items={[
          item({ id: "m1" }),
          item({ id: "m2", scope: "global", projectRoot: undefined, key: "tool", value: "rg" }),
        ]}
      />,
    );

    const mains = [...document.querySelectorAll(".history-main")].map((el) => el.textContent ?? "");
    expect(mains).toEqual([
      expect.stringContaining("workspace"),
      expect.stringContaining("rg"),
    ]);
    // Plain names, and the home folder shortened, never the account name.
    expect(screen.getAllByText("Preferred workspace")).toHaveLength(2);
    expect(screen.getByText(String.raw`Only in ~\app`)).toBeInTheDocument();
    expect(screen.getByText("Everywhere")).toBeInTheDocument();
    expect(screen.queryByText(/Users.Default/)).toBeNull();
    expect(screen.queryByText("preferred_cwd")).toBeNull();
    // Each Delete says which item it removes.
    expect(screen.getAllByRole("button", { name: "Delete" })[1]).toHaveAccessibleDescription(/tool.*rg/);

    await user.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    expect(onDelete).toHaveBeenCalledWith("m1");

    await user.click(screen.getAllByText("Preferred workspace")[0]);
    expect(onClose).not.toHaveBeenCalled();
    await user.click(document.querySelector(".settings-overlay")!);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
