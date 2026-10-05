import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { OutputView } from "./OutputView";

describe("OutputView", () => {
  it("makes each command's output a named region the keyboard can scroll", () => {
    render(
      <OutputView
        blocks={[
          {
            id: "e1",
            command: "Get-ChildItem logs",
            headline: "Finished. 4 lines of output.",
            output: "Directory: C:\\Work\\demo\\logs\na.log\nb.log",
          },
        ]}
        onClose={() => {}}
      />,
    );
    const region = screen.getByRole("region", { name: "Output of Get-ChildItem logs" });
    expect(region).toHaveAttribute("tabindex", "0");
    Object.defineProperty(region, "scrollHeight", { value: 400, configurable: true });
    Object.defineProperty(region, "clientHeight", { value: 40, configurable: true });
    region.focus();
    fireEvent.keyDown(region, { key: "End" });
    expect(region.scrollTop).toBe(400);
    fireEvent.keyDown(region, { key: "Home" });
    expect(region.scrollTop).toBe(0);
  });
});
