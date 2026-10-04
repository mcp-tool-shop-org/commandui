import { describe, expect, it } from "vitest";
import { recordedPlannerSource } from "./plannerSource";

describe("recordedPlannerSource", () => {
  it("keeps the engine the backend reported", () => {
    expect(recordedPlannerSource("mock")).toBe("mock");
    expect(recordedPlannerSource("ollama")).toBe("ollama");
  });

  it("does not invent a source", () => {
    expect(recordedPlannerSource("semantic")).toBeUndefined();
    expect(recordedPlannerSource("")).toBeUndefined();
  });
});
