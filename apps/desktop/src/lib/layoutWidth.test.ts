import { describe, expect, it } from "vitest";
import { NARROW_LAYOUT_PX, layoutIsNarrow, layoutWidth } from "./layoutWidth";

describe("layout width", () => {
  it("uses the smaller of the layout box and the painted box", () => {
    expect(layoutWidth(800, 1600)).toBe(800);
    expect(layoutWidth(1600, 800)).toBe(800);
    expect(layoutWidth(0, 0)).toBe(0);
  });

  it("is narrow under 960px and not before the box is measured", () => {
    expect(NARROW_LAYOUT_PX).toBe(960);
    expect(layoutIsNarrow(800)).toBe(true);
    expect(layoutIsNarrow(959)).toBe(true);
    expect(layoutIsNarrow(960)).toBe(false);
    expect(layoutIsNarrow(1600)).toBe(false);
    expect(layoutIsNarrow(0)).toBe(false);
  });
});
