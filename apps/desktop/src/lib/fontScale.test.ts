import { describe, expect, it } from "vitest";
import { fontScale, fontSizePercent } from "./fontScale";

describe("fontScale", () => {
  it("keeps a saved md at the current size", () => {
    expect(fontScale("md")).toBe(1);
    expect(fontScale("sm")).toBe(1);
    expect(fontScale(undefined)).toBe(1);
    expect(fontScale("nope")).toBe(1);
  });

  it("treats lg and the percent strings as a scale from 100 to 200", () => {
    expect(fontScale("lg")).toBe(2);
    expect(fontScale("200")).toBe(2);
    expect(fontScale("100")).toBe(1);
    expect(fontScale("150")).toBe(1.5);
    expect(fontScale("50")).toBe(1);
    expect(fontScale("400")).toBe(2);
    expect(fontSizePercent("lg")).toBe(200);
    expect(fontSizePercent("md")).toBe(100);
    expect(fontSizePercent("150")).toBe(150);
  });
});
