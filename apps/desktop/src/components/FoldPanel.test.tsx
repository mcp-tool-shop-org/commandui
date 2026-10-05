import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FoldPanel } from "./FoldPanel";

describe("FoldPanel", () => {
  it("starts open and hides its contents until shown again", async () => {
    const user = userEvent.setup();
    render(
      <FoldPanel hideLabel="Hide activity" showLabel="Show activity">
        <div role="region" aria-label="Activity text">
          Welcome to CommandUI
        </div>
      </FoldPanel>,
    );

    expect(screen.getByRole("region", { name: "Activity text" })).toBeVisible();
    const hide = screen.getByRole("button", { name: "Hide activity" });
    expect(hide).toHaveAttribute("aria-expanded", "true");

    await user.click(hide);
    expect(screen.getByRole("button", { name: "Show activity" })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("region", { name: "Activity text" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Show activity" }));
    expect(screen.getByRole("region", { name: "Activity text" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Hide activity" })).toHaveAttribute("aria-expanded", "true");
  });
});
